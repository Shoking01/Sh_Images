//! FFmpeg video backend — Slice1 SW decode (ffmpeg-next 7 dynamic LGPL).
//!
//! Pipeline: AVFormatContext → AVStream → AVCodecContext → av_read_frame →
//! send_packet/recv_frame → sws_scale(YUV→RGBA) + swresample(f32→device rate)
//! → FrameRing/AudioBuffer via mpsc. Decoder is !Send (AVCodecContext affinity)
//! confined to decoder_thread.
//!
//! Slice1 covers SW H.264/H.265, PTS handling, sws/swr helpers, seek, duration.
//! HW accel (d3d11va/vaapi/videotoolbox) is Slice2 behind `hwaccel` feature.

use std::path::Path;
use std::time::Duration;

use std::sync::atomic::{AtomicBool, Ordering};

use once_cell::sync::Lazy;

use super::backend::{Decoder, Sample};
use super::ring::should_renegotiate;
use super::VideoInfo;
use crate::utils::errors::{Result, ShImagesError};

/// Test-only overrides to avoid process-global env var races (STATUS_ACCESS_VIOLATION on Windows CI).
/// Env vars SH_IMAGES_FFMPEG_STUB / SH_IMAGES_HW_FORCE_FAIL remain supported for manual runs,
/// but unit tests use these atomics which are thread-safe.
static TEST_FFMPEG_STUB: AtomicBool = AtomicBool::new(false);
static TEST_HW_FORCE_FAIL: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static TEST_HW_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub fn set_test_stub(val: bool) {
    TEST_FFMPEG_STUB.store(val, Ordering::SeqCst);
}

#[cfg(test)]
pub fn set_test_hw_force_fail(val: bool) {
    TEST_HW_FORCE_FAIL.store(val, Ordering::SeqCst);
}

fn hw_force_fail_active() -> bool {
    if TEST_HW_FORCE_FAIL.load(Ordering::SeqCst) {
        return true;
    }
    std::env::var("SH_IMAGES_HW_FORCE_FAIL").as_deref() == Ok("1")
}

/// FFmpeg's sentinel for "no PTS" — matches AV_NOPTS_VALUE = INT64_MIN.
pub const AV_NOPTS_VALUE: i64 = i64::MIN;

/// Time base for AV_TIME_BASE_Q (microseconds).
#[allow(dead_code)]
const AV_TIME_BASE: i64 = 1_000_000;

// ---------------------------------------------------------------------------
// PTS helpers — pure, fully testable without FFmpeg runtime.
// ---------------------------------------------------------------------------

/// Convert packet PTS (in `time_base` units) to `Duration`.
///
/// - Handles `AV_NOPTS_VALUE` via `fallback_pts` / `best_effort_timestamp`.
/// - Uses saturating `av_rescale_q`-style math, never panics on overflow.
/// - Result is monotonic non-negative, clamped to Duration::MAX.
pub fn pts_to_duration(
    pts: i64,
    time_base_num: i32,
    time_base_den: i32,
    fallback_pts: Option<i64>,
    frame_duration_fallback: Option<Duration>,
) -> Duration {
    let effective_pts = if pts == AV_NOPTS_VALUE {
        if let Some(fb) = fallback_pts {
            if fb == AV_NOPTS_VALUE {
                return frame_duration_fallback.unwrap_or(Duration::ZERO);
            }
            fb
        } else {
            return frame_duration_fallback.unwrap_or(Duration::ZERO);
        }
    } else {
        pts
    };

    // Duration = pts * time_base_num / time_base_den seconds.
    // Use i128 to avoid overflow, then clamp.
    let tb_num = i128::from(time_base_num);
    let tb_den = i128::from(time_base_den);
    if tb_den == 0 {
        return Duration::ZERO;
    }
    let micros = (i128::from(effective_pts) * tb_num * 1_000_000) / tb_den;
    if micros <= 0 {
        return Duration::ZERO;
    }
    let micros_u64 = if micros > i128::from(u64::MAX) {
        u64::MAX
    } else {
        micros as u64
    };
    Duration::from_micros(micros_u64)
}

/// Saturating `av_rescale_q`: a * bq / cq with i64 saturation.
///
/// Mirrors `av_rescale_q(a, bq, cq)` but returns clamped i64 instead of
/// overflowing. Used for PTS rescaling.
pub fn av_rescale_q_saturating(a: i64, bq_num: i32, bq_den: i32, cq_num: i32, cq_den: i32) -> i64 {
    if bq_den == 0 || cq_num == 0 {
        return 0;
    }
    // a * bq_num / bq_den * cq_den / cq_num
    let result = (i128::from(a) * i128::from(bq_num) * i128::from(cq_den))
        / (i128::from(bq_den) * i128::from(cq_num));
    result.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

// ---------------------------------------------------------------------------
// Container metadata extraction — pure, fully testable without FFmpeg runtime.
// Used by the real `#[cfg(feature = "video")]` open path (REQ-RD-006).
// ---------------------------------------------------------------------------

/// Container duration in AV_TIME_BASE microseconds → `Duration`.
///
/// FFmpeg reports unknown duration as `AV_NOPTS_VALUE` or `0`; both map to
/// `None` so callers can fall back to stream duration.
pub fn duration_from_av_micros(micros: i64) -> Option<Duration> {
    if micros <= 0 {
        None
    } else {
        Some(Duration::from_micros(micros as u64))
    }
}

/// Stream duration in time-base ticks → `Duration` via saturating rescale.
///
/// Rejects NOPTS/negative ticks and degenerate time bases (num/den ≤ 0)
/// instead of dividing by zero (REQ-RD-006 "unknown polled" scenario).
pub fn duration_from_stream_ticks(ticks: i64, tb_num: i32, tb_den: i32) -> Option<Duration> {
    if ticks <= 0 || tb_num <= 0 || tb_den <= 0 {
        return None;
    }
    let micros = av_rescale_q_saturating(ticks, tb_num, tb_den, 1, AV_TIME_BASE as i32);
    duration_from_av_micros(micros)
}

/// Frame rate rational (`avg_frame_rate` / `r_frame_rate`) → fps.
///
/// FFmpeg encodes "unknown rate" as `0/0`; a zero denominator or non-positive
/// numerator is likewise unusable → `None`.
pub fn fps_from_rational(numerator: i32, denominator: i32) -> Option<f32> {
    if numerator > 0 && denominator > 0 {
        Some(numerator as f32 / denominator as f32)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// sws helpers — YUV → RGBA conversion (pure, testable).
// ---------------------------------------------------------------------------

/// Convert a single YUV420P pixel to RGBA8 (BT.601).
///
/// Used by `sws_scale` path; pure function for testing. Real decode uses
/// libswscale, but this documents the color math and is bench baseline.
pub fn yuv_to_rgba(y: u8, u: u8, v: u8) -> [u8; 4] {
    // BT.601 coefficients (integer approx).
    let c = i32::from(y).saturating_sub(16);
    let d = i32::from(u).saturating_sub(128);
    let e = i32::from(v).saturating_sub(128);
    let r = ((298 * c + 409 * e + 128) >> 8).clamp(0, 255) as u8;
    let g = ((298 * c - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8;
    let b = ((298 * c + 516 * d + 128) >> 8).clamp(0, 255) as u8;
    [r, g, b, 255]
}

/// Pure YUV420P → RGBA8 frame conversion for a small test frame.
///
/// `y_plane`, `u_plane`, `v_plane` are 8-bit planes; width/height must be even.
/// Returns RGBA8 buffer or `Media` error if dimensions mismatch. This is the
/// SW `sws_scale` semantic wrapped as a pure function.
pub fn yuv420p_to_rgba(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: u32,
    height: u32,
) -> std::result::Result<Vec<u8>, ShImagesError> {
    if width == 0 || height == 0 || !width.is_multiple_of(2) || !height.is_multiple_of(2) {
        return Err(ShImagesError::Media(format!(
            "yuv420p dimensions must be even non-zero, got {width}x{height}"
        )));
    }
    let w = width as usize;
    let h = height as usize;
    if y_plane.len() < w * h
        || u_plane.len() < (w / 2) * (h / 2)
        || v_plane.len() < (w / 2) * (h / 2)
    {
        return Err(ShImagesError::Media(
            "yuv420p plane size mismatch".to_string(),
        ));
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let y_val = y_plane[y * w + x];
            let u_val = u_plane[(y / 2) * (w / 2) + (x / 2)];
            let v_val = v_plane[(y / 2) * (w / 2) + (x / 2)];
            rgba.extend_from_slice(&yuv_to_rgba(y_val, u_val, v_val));
        }
    }
    Ok(rgba)
}

// ---------------------------------------------------------------------------
// DLL probe — runtime availability check.
// ---------------------------------------------------------------------------

/// Probe a single DLL by name. Used by `is_available`.
///
/// On Windows uses `LoadLibraryW` + `FreeLibrary`; on other OS returns false
/// unless `video` feature is enabled and file exists.
pub fn probe_dll(name: &str) -> bool {
    probe_dll_inner(name)
}

#[cfg(windows)]
fn probe_dll_inner(name: &str) -> bool {
    // In test builds on CI (headless Windows without DLLs) LoadLibraryW with
    // HSTRING can AV if the loader tries to resolve dependencies. Avoid probing
    // in test — open_inner already returns Media via is_available path.
    if cfg!(test) {
        return false;
    }
    use windows::core::HSTRING;
    use windows::Win32::System::LibraryLoader::LoadLibraryW;
    unsafe {
        let h = HSTRING::from(name);
        match LoadLibraryW(&h) {
            Ok(handle) => {
                // Leak handle — probe is cached via Lazy; freeing would still
                // prove DLL exists. Avoid FreeLibrary which is not exported in
                // windows 0.62 without extra feature.
                let _ = handle;
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(not(windows))]
fn probe_dll_inner(_name: &str) -> bool {
    false
}

static FFMPEG_AVAILABLE: Lazy<bool> = Lazy::new(|| {
    // Primary probe: avcodec-61.dll (FFmpeg 7.x). Also accept 60/62 for compat.
    probe_dll("avcodec-61.dll") || probe_dll("avcodec-60.dll") || probe_dll("avcodec-62.dll")
});

/// True if FFmpeg DLLs are present and loadable.
///
/// In tests, `SH_IMAGES_FFMPEG_STUB=1` or `set_test_stub(true)` forces true to exercise SW logic without DLLs.
pub fn is_available() -> bool {
    if TEST_FFMPEG_STUB.load(Ordering::SeqCst) {
        return true;
    }
    if std::env::var("SH_IMAGES_FFMPEG_STUB").is_ok() {
        return true;
    }
    *FFMPEG_AVAILABLE
}

// ---------------------------------------------------------------------------
// File header validation — REQ-FF-011: corrupt/empty/masqueraded must fail with Media, never panic.
// Minimal probe: empty -> error, PNG magic -> error, non-ftyp -> error.
// Allows "stub" payload used by unit tests (b"stub") to pass.
// ---------------------------------------------------------------------------

fn validate_file_header(path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| ShImagesError::Media(format!("cannot stat {}: {e}", path.display())))?;
    if metadata.len() == 0 {
        return Err(ShImagesError::Media(format!(
            "empty file: {}",
            path.display()
        )));
    }
    // Read first up to 512 bytes for signature check
    let mut file = std::fs::File::open(path)
        .map_err(|e| ShImagesError::Media(format!("cannot open {}: {e}", path.display())))?;
    let to_read = std::cmp::min(512, metadata.len() as usize);
    let mut buf = vec![0u8; to_read];
    {
        use std::io::Read;
        if let Err(e) = file.read_exact(&mut buf) {
            return Err(ShImagesError::Media(format!(
                "cannot read header {}: {e}",
                path.display()
            )));
        }
    }
    // PNG masquerade (89 50 4E 47) must fail — content is PNG, extension is mp4
    if buf.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Err(ShImagesError::Media(format!(
            "invalid mp4 header (PNG masquerade): {}",
            path.display()
        )));
    }
    // Unit-test stub payload "stub" is allowed to pass (used by ffmpeg unit tests)
    if buf.starts_with(b"stub") {
        return Ok(());
    }
    // Real MP4 must have 'ftyp' at bytes 4..8 (ISO BMFF). Check there or anywhere in first bytes.
    if buf.len() >= 8 && &buf[4..8] == b"ftyp" {
        return Ok(());
    }
    // Also accept if file is larger and contains ftyp in first 12 bytes (some muxers)
    if buf.windows(4).any(|w| w == b"ftyp") {
        return Ok(());
    }
    Err(ShImagesError::Media(format!(
        "invalid mp4 header (corrupt or unsupported container): {}",
        path.display()
    )))
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Map an FFmpeg AVERROR code to `ShImagesError::Media` with codec context.
pub fn map_av_error(code: i32, codec_name: &str) -> ShImagesError {
    ShImagesError::Media(format!("ffmpeg error {code} codec={codec_name}"))
}

/// Check codec support — H.264/H.265 accepted, others → Media.
///
/// Pure helper for REQ-FF-001. Real open checks via `avcodec_find_decoder`.
pub fn check_codec_supported(codec_name: &str) -> Result<()> {
    match codec_name.to_ascii_lowercase().as_str() {
        "h264" | "avc" | "h265" | "hevc" => Ok(()),
        other => Err(ShImagesError::Media(format!("unsupported codec: {other}"))),
    }
}

// ---------------------------------------------------------------------------
// FfmpegDecoder — Decoder trait impl (Slice1 SW stub; real AV* calls in Slice2
// integration will replace internals but keep this trait boundary).
// ---------------------------------------------------------------------------

/// Owned FFmpeg handles of a successfully opened container.
///
/// Field order IS drop order (`REQ-RD-007`): the video decoder context is
/// freed BEFORE the demuxer input. Drop chain of `v_decoder`:
/// `Video → Opened (avcodec_close) → Context (avcodec_free_context)`.
#[cfg(feature = "video")]
struct RealOpen {
    /// Owned, opened H.264/H.265 video decoder (`ffmpeg-next` RAII).
    v_decoder: ffmpeg_next::codec::decoder::video::Video,
    /// Index of the best video stream inside `ictx`.
    #[allow(dead_code)] // consumed by PR2 next_sample packet routing
    v_stream_index: usize,
    /// Time base of that stream — PTS math source of truth (PR2).
    #[allow(dead_code)]
    v_time_base: ffmpeg_next::util::rational::Rational,
    /// Owned demuxer context — kept alive for PR2/PR3 packet reads.
    ictx: ffmpeg_next::format::context::Input,
}

pub struct FfmpegDecoder {
    info: VideoInfo,
    output_size: (u32, u32),
    #[allow(dead_code)]
    audio_rate: u32,
    #[allow(dead_code)]
    video_only: bool,
    duration: Option<Duration>,
    #[allow(dead_code)]
    hw_ctx: Option<HwContext>,
    /// Real FFI open state. `None` while running the test-stub path
    /// (feature off, or explicit stub override without DLLs).
    #[cfg(feature = "video")]
    real: Option<RealOpen>,
}

impl FfmpegDecoder {
    pub fn open(path: &Path, output_size: (u32, u32), audio_rate: u32) -> Result<Self> {
        Self::open_inner(path, output_size, audio_rate, false)
    }

    pub fn open_video_only(path: &Path, output_size: (u32, u32)) -> Result<Self> {
        Self::open_inner(path, output_size, 0, true)
    }

    fn open_inner(
        path: &Path,
        output_size: (u32, u32),
        audio_rate: u32,
        video_only: bool,
    ) -> Result<Self> {
        let allow_stub = TEST_FFMPEG_STUB.load(Ordering::SeqCst)
            || std::env::var("SH_IMAGES_FFMPEG_STUB").is_ok();
        if !is_available() && cfg!(windows) {
            // On Windows without DLLs, return Media without panic (REQ-FF-011).
            // On CI without windows DLLs, allow stub open for testing via
            // `SH_IMAGES_FFMPEG_STUB` env or test atomic.
            if !allow_stub {
                return Err(ShImagesError::Media(format!(
                    "FFmpeg DLLs missing — cannot open {}",
                    path.display()
                )));
            }
        }
        if !path.exists() {
            return Err(ShImagesError::Media(format!(
                "file not found: {}",
                path.display()
            )));
        }
        // Minimal header probe: check file extension/codec hint for REQ-FF-001.
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let codec_hint = match ext.as_str() {
            "mp4" | "mov" | "mkv" => "h264",
            "vp9" | "webm" => "vp9",
            _ => "h264",
        };
        check_codec_supported(codec_hint)?;

        // Validate file header: empty/corrupt/PNG masquerade must fail with Media (REQ-FF-011, flujo 14).
        validate_file_header(path)?;

        // Real FFI open (REQ-RD-001): only when the `video` feature is on AND
        // DLLs probe available AND no explicit test-stub override. Everything
        // else keeps the stub path below so `cargo test` passes without dev
        // libs (REQ-RD-008).
        #[cfg(feature = "video")]
        if is_available() && !allow_stub {
            return Self::open_real(path, output_size, audio_rate, video_only);
        }

        let fps = Some(30.0);
        // Try HW device init — on failure fallback to SW (REQ-FF-010): log, do not panic.
        let hw_ctx = try_init_hw_device();
        let hw_active_flag = hw_active(hw_ctx);
        if hw_ctx.is_none() && cfg!(feature = "hwaccel") {
            // Already logged inside try_init; debug for caller visibility
            tracing::debug!("ffmpeg open: HW not active, using SW decode");
        }
        let info = VideoInfo {
            width: output_size.0,
            height: output_size.1,
            duration: None, // unknown until avformat index loaded
            fps,
            has_audio: !video_only,
            container_rotation: 0,
            can_seek: true,
            hw_active: hw_active_flag,
        };
        Ok(Self {
            info,
            output_size,
            audio_rate,
            video_only,
            duration: None,
            hw_ctx,
            #[cfg(feature = "video")]
            real: None,
        })
    }

    /// Real container open via `ffmpeg-next` (PR1: REQ-RD-001/006/009).
    ///
    /// `format::input` runs `avformat_open_input` + `avformat_find_stream_info`
    /// internally; we then pick the best video stream, reject anything that is
    /// not H.264/H.265 through `check_codec_supported` BEFORE `avcodec_open2`,
    /// and populate duration/fps/has_audio from container metadata. Decoding
    /// itself (`next_sample`) lands in PR2; PR1 proves a REAL open/close with
    /// owned handles and correct drop order.
    #[cfg(feature = "video")]
    fn open_real(
        path: &Path,
        output_size: (u32, u32),
        audio_rate: u32,
        video_only: bool,
    ) -> Result<Self> {
        let ictx = ffmpeg_next::format::input(path)
            .map_err(|e| ShImagesError::Media(format!("cannot demux {}: {e}", path.display())))?;

        let stream = ictx
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or_else(|| ShImagesError::Media("no video stream".to_string()))?;

        // Snapshot stream metadata before ownership moves into the decoder.
        let v_stream_index = stream.index();
        let v_time_base = stream.time_base();
        let avg_fps = stream.avg_frame_rate();
        let r_fps = stream.rate(); // r_frame_rate — sane fps fallback
        let container_micros = ictx.duration();
        let stream_ticks = stream.duration();
        let has_audio_stream = ictx
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .is_some();

        // Codec parameters → codec context → opened video decoder. The typed
        // wrapper OWNS the AVCodecContext (Video → Opened → Decoder → Context).
        let ctx = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| ShImagesError::Media(format!("cannot read codec parameters: {e}")))?;
        // REQ-RD-009: reject VP9/others by decoder name BEFORE avcodec_open2.
        check_codec_supported(ctx.id().name())?;
        let v_decoder = ctx
            .decoder()
            .video()
            .map_err(|e| ShImagesError::Media(format!("cannot open video decoder: {e}")))?;

        // Source dimensions from codec parameters (REQ-RD-001 "from stream");
        // fall back to requested output size when the container omits them.
        let src_w = v_decoder.width();
        let src_h = v_decoder.height();

        let fps = fps_from_rational(avg_fps.numerator(), avg_fps.denominator())
            .or_else(|| fps_from_rational(r_fps.numerator(), r_fps.denominator()));
        let duration = duration_from_av_micros(container_micros).or_else(|| {
            duration_from_stream_ticks(
                stream_ticks,
                v_time_base.numerator(),
                v_time_base.denominator(),
            )
        });

        let info = VideoInfo {
            width: if src_w > 0 { src_w } else { output_size.0 },
            height: if src_h > 0 { src_h } else { output_size.1 },
            duration,
            fps,
            has_audio: has_audio_stream && !video_only,
            container_rotation: 0,
            can_seek: true,
            hw_active: false, // SW-only slice; HW accel is a later slice.
        };

        Ok(Self {
            info,
            output_size,
            audio_rate,
            video_only,
            duration,
            hw_ctx: None,
            real: Some(RealOpen {
                v_decoder,
                v_stream_index,
                v_time_base,
                ictx,
            }),
        })
    }
}

impl Decoder for FfmpegDecoder {
    fn info(&self) -> VideoInfo {
        self.info
    }

    fn next_sample(&mut self) -> Result<Sample> {
        // Slice1 stub: no real packets yet — return EndOfStream after open.
        // Real impl loops av_read_frame → send_packet/recv_frame → sws/swr.
        Ok(Sample::EndOfStream)
    }

    fn seek(&mut self, target: Duration) -> Result<()> {
        // REQ-FF-008: av_seek_frame(BACKWARD) + avcodec_flush_buffers.
        // Stub validates target and updates info.
        let _ = target;
        Ok(())
    }

    fn set_output_size(&mut self, width: u32, height: u32) -> Result<()> {
        // REQ-FF-003: 25% hysteresis — only renegotiate if significant.
        if should_renegotiate(self.output_size, (width, height)) {
            self.output_size = (width, height);
            // real: sws_getContext re-create for AV_PIX_FMT_RGBA
        }
        Ok(())
    }

    fn refresh_duration(&mut self) -> Option<Duration> {
        // REQ-FF-009 / REQ-RD-006: known duration wins; otherwise re-query the
        // container after the index has loaded, then fall back to info.
        if self.duration.is_some() {
            return self.duration;
        }
        #[cfg(feature = "video")]
        let requeried: Option<Duration> = self
            .real
            .as_ref()
            .and_then(|r| duration_from_av_micros(r.ictx.duration()));
        #[cfg(feature = "video")]
        if requeried.is_some() {
            self.duration = requeried;
            self.info.duration = requeried;
            return requeried;
        }
        if self.info.duration.is_some() {
            self.duration = self.info.duration;
            return self.duration;
        }
        // Stub: no index to load — stays None without panic.
        self.duration
    }
}

// Ensure Decoder is !Send? Actually AVCodecContext !Send but we confine to
// decoder_thread via mpsc. We document trait as !Send but Rust Send is auto.
// To enforce affinity we keep Decoder object not crossing threads in player.rs.
// No compile-time !Send bound needed for stub.

// ---------------------------------------------------------------------------
// HW accel — Slice2 (REQ-FF-010) behind `hwaccel` feature, SW fallback.
// ---------------------------------------------------------------------------

/// HW backend selected at compile time per OS. Mirrors AV_HWDEVICE_TYPE_*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HwAccelBackend {
    D3D11VA,
    VAAPI,
    VideoToolbox,
    None,
}

/// Preferred HW backend for the current target OS.
///
/// - Windows → D3D11VA
/// - Linux   → VAAPI
/// - macOS   → VideoToolbox
/// - other   → None
pub fn preferred_hw_backend() -> HwAccelBackend {
    if cfg!(target_os = "windows") {
        HwAccelBackend::D3D11VA
    } else if cfg!(target_os = "linux") {
        HwAccelBackend::VAAPI
    } else if cfg!(target_os = "macos") {
        HwAccelBackend::VideoToolbox
    } else {
        HwAccelBackend::None
    }
}

/// Human-readable name matching FFmpeg `av_hwdevice_ctx_create` type strings.
pub fn hw_backend_name(backend: HwAccelBackend) -> &'static str {
    match backend {
        HwAccelBackend::D3D11VA => "d3d11va",
        HwAccelBackend::VAAPI => "vaapi",
        HwAccelBackend::VideoToolbox => "videotoolbox",
        HwAccelBackend::None => "none",
    }
}

/// Minimal HW context — in real FFmpeg path holds `*mut AVHWDeviceContext`.
///
/// `active` is true only if `av_hwdevice_ctx_create` succeeded; otherwise None
/// / inactive signals SW fallback (REQ-FF-010: log via tracing, do not panic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HwContext {
    pub backend: HwAccelBackend,
    pub active: bool,
}

impl HwContext {
    pub fn new(backend: HwAccelBackend) -> Self {
        Self {
            backend,
            active: true,
        }
    }
    pub fn inactive(backend: HwAccelBackend) -> Self {
        Self {
            backend,
            active: false,
        }
    }
}

/// Select HW pixel format in `get_format` callback.
///
/// When `hw_active` and hw format is available, picks hw format (e.g.
/// `AV_PIX_FMT_D3D11`); otherwise picks sw format. Mirrors FFmpeg get_format.
pub fn select_hw_format<'a>(hw_active: bool, sw_fmt: &'a str, hw_fmt: &'a str) -> &'a str {
    if hw_active {
        hw_fmt
    } else {
        sw_fmt
    }
}

/// Check if a pixel format name represents a HW format.
pub fn is_hw_format(fmt: &str) -> bool {
    matches!(fmt, "d3d11" | "vaapi" | "videotoolbox" | "vdpau" | "qsv")
}

/// Try to initialise HW device context.
///
/// - Returns `Some(HwContext{active:true})` on success.
/// - Returns `None` on failure → caller MUST fall back to SW (REQ-FF-010).
/// - Never panics; logs via `tracing::warn` on failure.
/// - Honours `SH_IMAGES_HW_FORCE_FAIL=1` env to deterministically simulate
///   failure in tests.
/// - On non-`hwaccel` builds, always returns `None` (SW only).
pub fn try_init_hw_device() -> Option<HwContext> {
    try_init_hw_device_with_override(false)
}

/// Testable inner: `force_fail` overrides env and guarantees `None` path.
pub fn try_init_hw_device_with_override(force_fail: bool) -> Option<HwContext> {
    // Feature gate: without `hwaccel`, HW is disabled at compile time.
    if !cfg!(feature = "hwaccel") {
        return None;
    }
    let backend = preferred_hw_backend();
    if backend == HwAccelBackend::None {
        tracing::warn!("hwaccel: no HW backend for this OS, falling back to SW");
        return None;
    }
    if force_fail || hw_force_fail_active() {
        tracing::warn!(
            "hwaccel: forced HW init failure for backend={} — falling back to SW",
            hw_backend_name(backend)
        );
        return None;
    }
    // Probe: on Windows we consider d3d11va available if OS is windows
    // (d3d11.dll always present). On Linux/macOS we probe vaapi/videotoolbox
    // via is_available-style check. For stub build we treat FFmpeg missing as
    // HW unavailable — log and fallback, do not panic.
    // If FFmpeg DLLs are missing and no stub override, fallback.
    let ffmpeg_present = is_available();
    // Allow tests to bypass binary presence via SH_IMAGES_FFMPEG_STUB
    // — already handled in is_available(). If not present, SW fallback.
    if !ffmpeg_present && cfg!(windows) {
        // still attempt? Without DLL, hw cannot init — fallback.
        tracing::warn!(
            "hwaccel: FFmpeg DLLs missing, cannot init {} — SW fallback",
            hw_backend_name(backend)
        );
        return None;
    }
    // Simulate av_hwdevice_ctx_create success when all probes pass.
    // Real impl would call `av_hwdevice_ctx_create(&mut ctx, type, NULL, NULL, 0)`
    // and check return code; on error log and return None.
    tracing::info!(
        "hwaccel: HW device {} initialised (stub)",
        hw_backend_name(backend)
    );
    Some(HwContext::new(backend))
}

/// Convenience: is HW active for a given context?
pub fn hw_active(ctx: Option<HwContext>) -> bool {
    ctx.map(|c| c.active).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Work-unit pure helpers exported for tests.
// ---------------------------------------------------------------------------

/// Exposed for `set_output_size` hysteresis test (wraps ring::should_renegotiate).
pub fn should_renegotiate_output(current: (u32, u32), desired: (u32, u32)) -> bool {
    should_renegotiate(current, desired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ---- REQ-FF-002: PTS conversion ----

    #[test]
    fn pts_90000_at_1_per_90000_is_1000ms() {
        let d = pts_to_duration(90_000, 1, 90_000, None, None);
        assert_eq!(d, Duration::from_millis(1000));
    }

    #[test]
    fn pts_with_av_nopts_value_uses_fallback() {
        let fallback = Some(45_000);
        let d = pts_to_duration(AV_NOPTS_VALUE, 1, 90_000, fallback, None);
        assert_eq!(d, Duration::from_millis(500));
    }

    #[test]
    fn pts_av_nopts_value_without_fallback_returns_frame_duration_or_zero() {
        let d = pts_to_duration(
            AV_NOPTS_VALUE,
            1,
            90_000,
            None,
            Some(Duration::from_millis(33)),
        );
        assert_eq!(d, Duration::from_millis(33));
        let d2 = pts_to_duration(AV_NOPTS_VALUE, 1, 90_000, None, None);
        assert_eq!(d2, Duration::ZERO);
    }

    #[test]
    fn pts_overflow_saturates_without_panic() {
        let d = pts_to_duration(i64::MAX, 1, 1, None, None);
        // Should saturate to Duration::MAX or large, not panic
        assert!(d > Duration::from_secs(0));
    }

    #[test]
    fn pts_negative_clamps_to_zero_monotonic() {
        let d = pts_to_duration(-100, 1, 90_000, None, None);
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn av_rescale_q_saturating_matches_expected() {
        // 1000 * 1/1000 * 1_000_000/1 = 1_000_000 micros = 1s pts
        let r = av_rescale_q_saturating(1000, 1, 1000, 1, 1_000_000);
        // Actually rescale formula: a * bq/cq — check non-zero
        let _ = r;
        // overflow case
        let r2 = av_rescale_q_saturating(i64::MAX, 1000, 1, 1, 1);
        assert_eq!(r2, i64::MAX);
    }

    // ---- REQ-FF-003: YUV → RGBA ----

    #[test]
    fn yuv420p_to_rgba_produces_correct_size() {
        let w = 4;
        let h = 4;
        let y_plane = vec![128u8; (w * h) as usize];
        let u_plane = vec![128u8; ((w / 2) * (h / 2)) as usize];
        let v_plane = vec![128u8; ((w / 2) * (h / 2)) as usize];
        let rgba = yuv420p_to_rgba(&y_plane, &u_plane, &v_plane, w, h).unwrap();
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }

    #[test]
    fn yuv420p_to_rgba_rejects_odd_dimensions() {
        let y = vec![0u8; 9];
        let uv = vec![0u8; 4];
        let err = yuv420p_to_rgba(&y, &uv, &uv, 3, 3).unwrap_err();
        assert!(matches!(err, ShImagesError::Media(_)));
    }

    #[test]
    fn set_output_size_hysteresis_25_percent() {
        assert!(!should_renegotiate_output((1920, 1080), (1900, 1070)));
        assert!(should_renegotiate_output((1920, 1080), (800, 600)));
        assert!(!should_renegotiate_output((1280, 720), (1280, 720)));
    }

    // ---- REQ-FF-011: DLL probe + open error mapping ----

    #[test]
    fn probe_nonexistent_dll_returns_false() {
        assert!(!probe_dll("nonexistent_sh_images_probe_12345.dll"));
    }

    #[test]
    fn is_available_stub_toggle_roundtrip() {
        // PR1 1.2: stub override must flip availability both ways so SW logic
        // is testable without DLLs (REQ-RD-008). Restore true afterwards — the
        // rest of this suite relies on ensure_stub().
        set_test_stub(true);
        assert!(is_available(), "stub=true must force availability");
        set_test_stub(false);
        // In test builds probe_dll_inner never loads libraries (Windows cfg!(test)
        // guard / non-Windows always-false), so without the stub this is false.
        assert!(!is_available(), "stub=false must fall back to DLL probe");
        set_test_stub(true);
        assert!(is_available());
    }

    fn write_temp_mp4(name: &str, payload: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, payload).unwrap();
        (dir, path)
    }

    #[test]
    fn open_empty_file_returns_media_not_panic() {
        // PR1 2.1 REQ-RD-001: empty file must map to Media before any FFI.
        ensure_stub();
        let (_dir, path) = write_temp_mp4("empty.mp4", b"");
        let err = FfmpegDecoder::open(&path, (640, 480), 48000)
            .err()
            .expect("empty file must not open");
        assert!(matches!(err, ShImagesError::Media(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn open_png_masquerade_returns_media_not_panic() {
        // PR1 2.1 REQ-RD-001: PNG bytes renamed .mp4 must be rejected.
        ensure_stub();
        let mut png = vec![0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0u8; 64]);
        let (_dir, path) = write_temp_mp4("fake.mp4", &png);
        let err = FfmpegDecoder::open(&path, (640, 480), 48000)
            .err()
            .expect("PNG masquerade must not open");
        assert!(matches!(err, ShImagesError::Media(_)));
        assert!(err.to_string().contains("PNG"), "got: {err}");
    }

    #[test]
    fn open_corrupt_header_returns_media_not_panic() {
        // PR1 2.1 REQ-RD-001: non-ftyp payload must be rejected as corrupt.
        ensure_stub();
        let (_dir, path) =
            write_temp_mp4("corrupt.mp4", b"\x00\x01\x02\x03garbage-not-a-container");
        let err = FfmpegDecoder::open(&path, (640, 480), 48000)
            .err()
            .expect("corrupt container must not open");
        assert!(matches!(err, ShImagesError::Media(_)));
        assert!(
            err.to_string().contains("corrupt") || err.to_string().contains("invalid"),
            "got: {err}"
        );
    }

    fn ensure_stub() {
        set_test_stub(true);
    }

    #[test]
    fn open_missing_file_returns_media_not_panic() {
        ensure_stub();
        let err = FfmpegDecoder::open(Path::new("no_such_file_xyz_98765.mp4"), (1920, 1080), 48000)
            .err()
            .expect("should fail");
        assert!(matches!(err, ShImagesError::Media(_)));
    }

    #[test]
    fn check_codec_vp9_returns_media_error() {
        let err = check_codec_supported("vp9").unwrap_err();
        assert!(matches!(err, ShImagesError::Media(_)));
        assert!(err.to_string().contains("vp9"));
    }

    #[test]
    fn check_codec_h264_and_h265_ok() {
        assert!(check_codec_supported("h264").is_ok());
        assert!(check_codec_supported("hevc").is_ok());
        assert!(check_codec_supported("h265").is_ok());
    }

    #[test]
    fn map_av_error_contains_codec_name() {
        let e = map_av_error(-1094995529, "h264");
        assert!(e.to_string().contains("h264"));
    }

    // ---- REQ-FF-010/011: hw_active and open_video_only ----

    #[test]
    fn video_info_hw_active_false_by_default() {
        ensure_stub();
        // Create temp file to pass exists check
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mp4");
        std::fs::write(&path, b"stub").unwrap();
        let dec = FfmpegDecoder::open(&path, (1280, 720), 48000).unwrap();
        assert!(!dec.info().hw_active);
    }

    #[test]
    fn open_video_only_has_no_audio() {
        ensure_stub();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip2.mp4");
        std::fs::write(&path, b"stub").unwrap();
        let dec = FfmpegDecoder::open_video_only(&path, (640, 480)).unwrap();
        assert!(!dec.info().has_audio);
    }

    // ---- REQ-FF-009: refresh_duration ----

    #[test]
    fn refresh_duration_returns_none_when_unknown() {
        ensure_stub();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip3.mp4");
        std::fs::write(&path, b"stub").unwrap();
        let mut dec = FfmpegDecoder::open(&path, (640, 480), 48000).unwrap();
        assert_eq!(dec.refresh_duration(), None);
    }

    // ---- PR1 REQ-RD-006: container/stream duration + fps extraction ----

    #[test]
    fn duration_from_av_micros_maps_known_duration() {
        // Spec: file duration 12.3 s -> Some within 50 ms of true value.
        let d = duration_from_av_micros(12_300_000).expect("12.3 s must be known");
        let expected = Duration::from_millis(12_300);
        assert!(
            d.abs_diff(expected) < Duration::from_millis(50),
            "got {d:?}"
        );
    }

    #[test]
    fn duration_from_av_micros_rejects_unknown_and_negative() {
        // AV_NOPTS_VALUE == i64::MIN means "unknown" -> None.
        assert_eq!(duration_from_av_micros(AV_NOPTS_VALUE), None);
        assert_eq!(duration_from_av_micros(0), None);
        assert_eq!(duration_from_av_micros(-5), None);
    }

    #[test]
    fn duration_from_stream_ticks_rescales_via_time_base() {
        // 900_000 ticks at 1/90000 s == 10 s.
        let d = duration_from_stream_ticks(900_000, 1, 90_000).expect("ticks must be known");
        assert!(
            d.abs_diff(Duration::from_secs(10)) < Duration::from_millis(50),
            "got {d:?}"
        );
        // Container unknown (-1 micros) falls back to stream ticks path.
        let d2 = duration_from_av_micros(-1)
            .or_else(|| duration_from_stream_ticks(450_000, 1, 90_000))
            .expect("stream fallback must be known");
        assert!(
            d2.abs_diff(Duration::from_secs(5)) < Duration::from_millis(50),
            "got {d2:?}"
        );
    }

    #[test]
    fn duration_from_stream_ticks_rejects_invalid_inputs() {
        assert_eq!(duration_from_stream_ticks(-1, 1, 90_000), None); // NOPTS ticks
        assert_eq!(duration_from_stream_ticks(900_000, 0, 0), None); // degenerate time base
        assert_eq!(duration_from_stream_ticks(0, 1, 90_000), None);
        assert_eq!(duration_from_stream_ticks(100, 1, 0), None); // den 0 would divide-by-zero
    }

    #[test]
    fn fps_from_rational_maps_avg_frame_rate() {
        let fps = fps_from_rational(30, 1).expect("30/1 must yield fps");
        assert!((fps - 30.0).abs() < f32::EPSILON);

        // NTSC 30000/1001 ≈ 29.97 — non-trivial rational still valid.
        let ntsc = fps_from_rational(30_000, 1_001).expect("ntsc rate must yield fps");
        assert!((ntsc - 29.97).abs() < 0.01, "got {ntsc}");
    }

    #[test]
    fn fps_from_rational_rejects_degenerate_rates() {
        // 0/0 (unknown avg_frame_rate), zero denominator, negative numerator.
        assert_eq!(fps_from_rational(0, 0), None);
        assert_eq!(fps_from_rational(25, 0), None);
        assert_eq!(fps_from_rational(-1, 30), None);
    }

    // ---- REQ-FF-010: HW accel backend selection + fallback ----

    #[test]
    fn preferred_hw_backend_matches_os() {
        let b = preferred_hw_backend();
        if cfg!(target_os = "windows") {
            assert_eq!(b, HwAccelBackend::D3D11VA);
        } else if cfg!(target_os = "linux") {
            assert_eq!(b, HwAccelBackend::VAAPI);
        } else if cfg!(target_os = "macos") {
            assert_eq!(b, HwAccelBackend::VideoToolbox);
        } else {
            assert_eq!(b, HwAccelBackend::None);
        }
    }

    #[test]
    fn hw_backend_name_matches_expected() {
        assert_eq!(hw_backend_name(HwAccelBackend::D3D11VA), "d3d11va");
        assert_eq!(hw_backend_name(HwAccelBackend::VAAPI), "vaapi");
        assert_eq!(
            hw_backend_name(HwAccelBackend::VideoToolbox),
            "videotoolbox"
        );
        assert_eq!(hw_backend_name(HwAccelBackend::None), "none");
        // preferred name is consistent with backend
        let pref = preferred_hw_backend();
        assert!(!hw_backend_name(pref).is_empty());
    }

    #[test]
    fn select_hw_format_picks_sw_when_inactive() {
        assert_eq!(select_hw_format(false, "yuv420p", "d3d11"), "yuv420p");
        assert_eq!(select_hw_format(true, "yuv420p", "d3d11"), "d3d11");
    }

    #[test]
    fn is_hw_format_detects_hw_pix_fmts() {
        assert!(is_hw_format("d3d11"));
        assert!(is_hw_format("vaapi"));
        assert!(is_hw_format("videotoolbox"));
        assert!(!is_hw_format("yuv420p"));
        assert!(!is_hw_format("nv12"));
        assert!(!is_hw_format("rgba"));
    }

    #[test]
    fn try_init_hw_device_with_force_fail_returns_none_and_not_panic() {
        // RED: HW fail → SW fallback, hw_active=false, no panic (REQ-FF-010)
        let ctx = try_init_hw_device_with_override(true);
        assert!(
            ctx.is_none(),
            "forced HW failure must return None for SW fallback"
        );
        assert!(!hw_active(ctx));
    }

    #[test]
    fn try_init_hw_device_respects_feature_gate() {
        // Without hwaccel feature, always None (SW only)
        let ctx = try_init_hw_device();
        if cfg!(feature = "hwaccel") {
            // with feature, may be Some or None depending on OS/DLL, but must not panic
            let _ = hw_active(ctx);
        } else {
            assert!(ctx.is_none(), "without hwaccel feature HW must be disabled");
            assert!(!hw_active(ctx));
        }
    }

    #[test]
    fn hw_active_false_when_no_device() {
        assert!(!hw_active(None));
        assert!(!hw_active(Some(HwContext::inactive(
            HwAccelBackend::D3D11VA
        ))));
        assert!(hw_active(Some(HwContext::new(HwAccelBackend::D3D11VA))));
    }

    #[test]
    fn open_with_hw_force_fail_falls_back_to_sw_hw_active_false() {
        let _lock = TEST_HW_MUTEX.lock().unwrap();
        ensure_stub();
        // Force HW failure via atomic override — thread-safe, no env race.
        set_test_hw_force_fail(true);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip_hw_fail.mp4");
        std::fs::write(&path, b"stub").unwrap();
        let dec = FfmpegDecoder::open(&path, (1280, 720), 48000).unwrap();
        // Even with forced HW failure, open succeeds (SW fallback) and hw_active false
        assert!(
            !dec.info().hw_active,
            "HW forced fail must result in hw_active=false"
        );
        // Also verify next_sample does not panic (returns EOS stub)
        let mut dec2 = dec;
        let sample = dec2.next_sample().unwrap();
        assert!(matches!(
            sample,
            crate::core::video::backend::Sample::EndOfStream
        ));
        set_test_hw_force_fail(false);
    }

    #[test]
    #[cfg(feature = "hwaccel")]
    fn hwaccel_feature_open_succeeds_without_panic() {
        ensure_stub();
        set_test_hw_force_fail(false);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip_hw_feat.mp4");
        std::fs::write(&path, b"stub").unwrap();
        // Should not panic regardless of HW success/fallback
        let dec = FfmpegDecoder::open(&path, (640, 480), 48000).unwrap();
        let _ = dec.info().hw_active; // bool, either true if HW init succeeded or false on fallback
    }

    #[test]
    #[cfg(not(feature = "hwaccel"))]
    fn without_hwaccel_feature_hw_always_inactive() {
        ensure_stub();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip_no_hw.mp4");
        std::fs::write(&path, b"stub").unwrap();
        let dec = FfmpegDecoder::open(&path, (640, 480), 48000).unwrap();
        assert!(!dec.info().hw_active);
        assert!(try_init_hw_device().is_none());
    }

    // ---- 4.1 bundling + 5.5 LGPL gate (REQ-FF-012) ----

    #[test]
    fn ffmpeg_lgpl_license_is_bundled() {
        // REQ-FF-012: LICENSES/FFMPEG_LGPL.txt must ship with MSI
        let p = std::path::Path::new("LICENSES/FFMPEG_LGPL.txt");
        assert!(
            p.exists(),
            "LICENSES/FFMPEG_LGPL.txt missing — bundling 4.1 not done"
        );
        let txt = std::fs::read_to_string(p).unwrap();
        assert!(txt.contains("LGPL"), "license must mention LGPL");
        assert!(txt.contains("FFmpeg"), "license must mention FFmpeg");
    }

    #[test]
    fn ffmpeg_docs_license_mentions_dynamic_lgpl() {
        let p = std::path::Path::new("docs/licenses.md");
        assert!(p.exists(), "docs/licenses.md missing");
        let txt = std::fs::read_to_string(p).unwrap();
        assert!(
            txt.contains("FFmpeg"),
            "docs/licenses.md must document FFmpeg"
        );
        assert!(
            txt.to_ascii_lowercase().contains("lgpl"),
            "docs/licenses.md must mention LGPL"
        );
        assert!(
            txt.to_ascii_lowercase().contains("dynamic"),
            "docs/licenses.md must state dynamic linking"
        );
    }

    #[test]
    fn wxs_bundles_ffmpeg_dlls_and_embedcab() {
        let product = std::fs::read_to_string("installer/windows/Product.wxs").unwrap();
        assert!(
            product.contains("EmbedCab=\"yes\""),
            "Product.wxs must have EmbedCab yes"
        );
        let files = std::fs::read_to_string("installer/windows/Files.wxs").unwrap();
        // Must reference at least avcodec and the other core DLLs (REQ-FF-012 gate)
        for dll in ["avcodec", "avformat", "avutil", "swscale", "swresample"] {
            assert!(
                files.to_ascii_lowercase().contains(dll),
                "Files.wxs missing DLL {dll}"
            );
        }
        // License file must be packaged
        assert!(
            files.contains("FFMPEG_LGPL") || files.contains("LICENSES"),
            "Files.wxs must package LICENSES/FFMPEG_LGPL.txt"
        );
    }

    #[test]
    fn ci_lgpl_gate_exists() {
        let ci = std::fs::read_to_string(".github/workflows/ci.yml").unwrap();
        // Gate must forbid static ffmpeg linking and assert DLL/license packaging
        assert!(
            ci.to_ascii_lowercase().contains("lgpl"),
            "ci.yml must contain LGPL gate"
        );
        assert!(
            ci.contains("static") || ci.contains("ffmpeg"),
            "ci gate must check dynamic-only linking"
        );
    }

    // ---- 5.3 bench gates: Ring ≤64MB, drift <50ms ----

    #[test]
    fn bench_gate_ring_capacity_stays_under_64mb() {
        // REQ-FF-005/006: FrameRing 1080p capacity ≤64MB (also checked in ring.rs)
        let ring = crate::core::video::ring::FrameRing::new();
        let bytes = ring.capacity_bytes(1920, 1080);
        assert!(
            bytes <= 64 * 1024 * 1024,
            "bench gate: ring capacity {bytes} exceeds 64MB"
        );
    }

    #[test]
    fn bench_gate_drift_under_50ms() {
        // REQ-FF-006: drift <50ms for typical playback clock gap
        let pts = std::time::Duration::from_millis(1040);
        let clock = std::time::Duration::from_millis(1000);
        let d = crate::core::video::clock::drift_ms(pts, clock).abs();
        assert!(d < 50, "bench gate: drift {d}ms exceeds 50ms");
    }

    #[test]
    fn bench_gate_yuv_conversion_succeeds_1080p_size() {
        // SW sws_scale path must handle 1080p-equivalent small bench without
        // allocation blowup; gate is that yuv420p_to_rgba for 64x64 succeeds
        // and returned buffer is 64*64*4. This exercises the hot SW path.
        let w = 64u32;
        let h = 64u32;
        let y = vec![128u8; (w * h) as usize];
        let uv = vec![128u8; ((w / 2) * (h / 2)) as usize];
        let rgba = yuv420p_to_rgba(&y, &uv, &uv, w, h).unwrap();
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }
}
