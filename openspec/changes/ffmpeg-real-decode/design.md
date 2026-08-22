# Design: ffmpeg-real-decode

## Technical Approach

Replace stub `FfmpegDecoder::next_sample()->EndOfStream` with real `ffmpeg-next 7` dynamic SW decode. Keep `Decoder` trait and `player::decoder_thread` protocol unchanged; reuse pure helpers (`pts_to_duration`, `av_rescale_q_saturating`, `validate_file_header`, `map_av_error`, `probe_dll`/`is_available`, `check_codec_supported`) and pure stack (`FrameRing` 6 slots/64 MB, `AudioBuffer` 1.5 s, `PlayClock`, `seek::discard_before_seek_target`, `yuv_to_rgba` bench baseline). Own handles via safe wrappers; `default=[]`, no `static`, no new threads.

## Architecture Decisions

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `ffmpeg-next 7` dynamic vs raw `ffmpeg-sys-next`/`rsmpeg` | Raw: direct `AVHWDeviceContext` but more `unsafe`, manual refcount, re-derive `Rational` | **Keep `ffmpeg-next 7`** — safe wrappers justify existing tests, LGPL bundling already solved, pkg-config cross-platform |
| Owned wrappers vs raw `*mut AV*` | Raw leaks near 64 MB budget if `av_frame_free` missed | **Owned** `Input`/`Context`/`scaling::Context`/`resampling::Context`/`Packet`/`Video`/`Audio` — RAII `Drop`, `!Send` confined to `decoder_thread` |
| `SwsContext`/`SwrContext` per-decoder vs per-frame | Per-frame ~2 ms at 1080p; stale stride on resize | **One each per decoder**, recreate only on `should_renegotiate`; freed before codec ctx in `Drop` |
| `AVPacket`/`AVFrame` reuse vs alloc/sample | 30 allocs/s + leak on early return | **Reuse one `Packet` + two `Frames`**, `unref` each loop |

## Data Flow

```
Path → avformat_open_input + find_stream_info → streams[video(idx,tb,avg_fps), audio(idx,tb)]
  ├─ codec::decoder::find→open2 → AVCodecContext(video) → send_packet/receive_frame --EAGAIN--> sws_scale YUV→RGBA → ring.take_buffer → VideoFrame → FrameRing::push(epoch) → Sample::Video
  └─ codec::decoder::find→open2 → AVCodecContext(audio) → send_packet/receive_frame ---------> swr_convert → FLT @audio_rate → Sample::Audio → AudioBuffer
PTS: store Rational(num,den); pts_to_duration(pts,tb,fallback,best_effort) i128 saturate, negative→ZERO.
Seek: player debounces 120ms coalesces → decoder.seek(target): av_seek_frame(BACKWARD, av_rescale_q_saturating(target, AV_TIME_BASE_Q, stream_tb)) + avcodec_flush_buffers(video+audio); player discards pts<target via discard_before_seek_target.
```

`next_sample` loop:

```
loop {
  packet = av_read_frame(ictx) // None → drain → EndOfStream
  if packet.stream_index==v_idx { v_ctx.send_packet(&packet); } else if packet.stream_index==a_idx && !video_only { a_ctx.send_packet... }
  match receive_frame { Ok(f)→ sws/swr → return Sample, Err(EAGAIN)→ continue, Err(EOF)→ drain }
}
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/core/video/ffmpeg.rs` | Modify | Real `FfmpegDecoder` fields, `open_inner`, `next_sample`, `seek`, `refresh_duration`, `Drop`; `#[cfg(feature="video")]` real path gated by `is_available()`, else stub `Media` |
| `src/core/video/backend.rs` | Modify | Docs MF→FFmpeg only |
| `src/core/video/player.rs` | Modify | Comments MF→FFmpeg; keep `ring.is_full()\|\|audio.is_full()` backpressure |
| `Cargo.toml` | Verify | `default=[]`, `video=["dep:ffmpeg-next"]`, no `static` |
| `installer/windows/Files.wxs` | Verify | `avcodec-61.dll` etc + `EmbedCab="yes"` |
| `tests/common/mod.rs` + `tests/video_decode.rs` | Modify/Create | <100 KB fixture via lavfi or tiny committed mp4, gated `#[cfg(feature="video")]` + `is_available()` |
| `.github/workflows/ci.yml` | Modify | Add `cargo test --features video` lane with dev libs; default `cargo test` stays without `video` |

### Ownership

```rust
#[cfg(feature="video")]
pub struct FfmpegDecoder {
  ictx: format::context::Input, // AVFormatContext
  v_ctx: Option<codec::context::Context>, a_ctx: Option<codec::context::Context>,
  scaler: Option<software::scaling::Context>, resampler: Option<software::resampling::Context>,
  v_stream: (usize, Rational), a_stream: Option<(usize, Rational)>,
  packet: packet::Packet, v_frame: frame::Video, a_frame: frame::Audio,
  duration: Option<Duration>, output_size: (u32,u32), audio_rate: u32, video_only: bool,
  // Drop: scaler/resampler → v_ctx/a_ctx → ictx; frames/packet unref auto
}
```

Open: `validate_file_header` pre-check → `format::input(&path)` → `find_stream_info` → streams filter `media_type==Video/Audio` → `check_codec_supported` (reject vp9/webm) → `find`/`open2` → `SwsContext::get` (YUV420P/NV12→RGBA) / `SwrContext` (any→FLT, layout from stream, `audio_rate`). `duration` from `ictx.duration()`/`stream.duration` via `av_rescale_q_saturating`; `fps` from `avg_frame_rate`; `has_audio` from discovery.

## Interfaces / Contracts

- `Decoder` unchanged: `open`, `open_video_only`, `info()`, `next_sample()->Sample`, `seek`, `set_output_size` (hysteresis 25%), `refresh_duration`.
- `Sample::Video{pts,w=output_size capped 1920,h capped 1080,pixels.len==w*h*4}` via `ring.take_buffer`.
- `Sample::Audio{pts,samples:Vec<f32>}` interleaved FLT at `audio_rate`; never emitted if `video_only` or no audio stream.
- `is_available()` probes `avcodec-61|60|62.dll` (Lazy+atomics, `TEST_FFMPEG_STUB` backdoor); `!is_available()` → `Err(Media("FFmpeg DLLs missing"))` without FFI.

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `pts_to_duration`, `av_rescale_q_saturating`, `yuv_to_rgba`, `validate_file_header`, `map_av_error`, `check_codec_supported`, `probe_dll` | Existing 30 tests + `NOPTS fallback`, `overflow saturate`, `EAGAIN loop`, `Drop no panic` via `set_test_stub(true)` |
| Integration | Real RGBA + resampled audio + seek + duration | `tests/video_decode.rs` `#[cfg(feature="video")]`, early-return if `!is_available()`; fixture `ffmpeg -f lavfi color=c=red:s=64x64:r=30:d=1` or tiny mp4; assert `w*h*4`, `pts>=ZERO`, non-empty f32 |
| Stub gate | `cargo test` without `video` passes | Exercises `validate_file_header→Media`; no link to `ffmpeg-sys-next` |
| Video gate | `cargo test --features video` | CI installs `libav*`/winget ffmpeg; clippy `-D warnings` green |
| Budget | Ring ≤64 MB, no leak | `capacity_bytes(1920,1080)<=64MB` + Drop after 60 frames |

## Threat Matrix

N/A — no routing, shell, subprocess, VCS/PR automation, executable-file classification, or process-integration boundary.

## Migration / Rollout

No migration. `video` opt-in. Rollback: revert `ffmpeg.rs` to stub, `is_available()==false` ships, delete >100 KB fixtures. Pin `ffmpeg-next="7"` + probe `60|61|62`; CI `static` gate stays.

## Open Questions

- [ ] Fixture: commit <100 KB mp4 vs lavfi generation (needs `ffmpeg` on CI) — ADR before tasks
- [ ] Audio `channel_layout` mono/5.1→stereo downmix policy
