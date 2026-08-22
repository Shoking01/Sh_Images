# Exploration: ffmpeg-real-decode

> Change: `ffmpeg-real-decode` — wire real FFI decode so H.264 mp4 renders frames and audio resamples.
> Date: 2026-08-21 · Mode: `openspec` (also snapshot to Engram optionally) · Author: sdd-explore sub-agent

## Goal

Replace the Slice1 stub in `src/core/video/ffmpeg.rs` (which opens but always returns `EndOfStream`) with a real `ffmpeg-next 7.x` dynamic SW decode that feeds flat `Sample::Video/Audio` into the existing `FrameRing` (64 MB cap), `AudioBuffer`, `PlayClock`, seek and timeline machinery, without changing public `Decoder` trait or player `decoder_thread` protocol.

## Current State

**What shipped in archived `ffmpeg-video-backend` (18/18 tasks, 2026-08-20):**

- `src/core/video/ffmpeg.rs` (999 lines) is a **pure+stub** hybrid:
  - Pure, fully tested helpers: `pts_to_duration`, `av_rescale_q_saturating`, `yuv_to_rgba`/`yuv420p_to_rgba` (BT.601), `probe_dll`/`is_available` (LoadLibraryW, Lazy + atomics), `map_av_error`, `check_codec_supported`, `validate_file_header` (empty/PNG-masquerade/ftyp probe), HW helpers (`preferred_hw_backend`, `hw_backend_name`, `select_hw_format`, `is_hw_format`, `try_init_hw_device_with_override`, `hw_active`), and `should_renegotiate_output`.
  - Stub struct `FfmpegDecoder { info, output_size, audio_rate, video_only, duration, hw_ctx }`. `open_inner` checks `is_available` + `path.exists()` + codec hint by extension + `validate_file_header` + `try_init_hw_device` (logs fallback). `next_sample` returns `Ok(EndOfStream)` always. `seek`/`set_output_size`/`refresh_duration` are no-ops/stored-state. `AV_NOPTS_VALUE`, `AV_TIME_BASE` constants already defined. HW `Slice2` types (`HwAccelBackend`, `HwContext`) are behind `hwaccel` feature gate.
  - 30+ unit tests exercise the pure helpers and the stub open path via `set_test_stub(true)` / `set_test_hw_force_fail` atomics (avoids env races that AV'd on Windows CI). Three LGPL/bundling gates and three bench gates are already green.

- `src/core/video/backend.rs`: All platforms now route through `ffmpeg_platform` → `FfmpegDecoder`. The old `stub` module for non-Windows is dead code (kept for rollback comparison, deleted Slice3). `Decoder` trait is not `Send` by convention (COM affinity) but no compile-time bound; confinement is via `decoder_thread` + `mpsc` flat `Sample`.

- `src/core/video/player.rs` (`decoder_thread`, 837 lines): Already ffmpeg-ready in protocol but comments still reference MF. Flow: spawn `audio_sample_rate_probe` (now early-returns 0 in `cfg(test)`/`CI`), `backend::open`, publish `info`/`ready`, optional `start_audio_stream` (cpal), then loop of `try_recv` commands, `is_full` backpressure (`ring.is_full() || audio.is_full()`), `ring.epoch()` capture, `decoder.next_sample()` match (`push` video / `push` audio / `EndOfStream`→`reached_end` / error). `apply_seek` flushes audio and bumps epoch. `shutdown` is `AtomicBool`.

- `src/core/video/ring.rs` (601 lines), `audio.rs` (320), `clock.rs` (272), `timeline.rs`, `seek.rs`, `playback.rs`, `volume.rs`, `format.rs`: All pure, unchanged by backend choice, within budgets (`RING_CAPACITY=6` → ~49.7 MiB at 1080p, `capacity_bytes_stays_under_the_budget_at_1080p` gate). Must be kept as-is.

- `Cargo.toml`: `default = []`, `video = ["dep:ffmpeg-next"]` (optional 7.x dynamic LGPL), `hwaccel = ["video"]`, MSRV 1.92, release `lto/strip` for <20 MB binary. `ffmpeg-sys-next` is not `static`. Probe handles `avcodec-61.dll` (also 60/62 compat).

- `installer/windows/Files.wxs` + `Product.wxs`: Already bundle `avcodec-61.dll`, `avformat-61.dll`, `avutil-59.dll`, `swscale-8.dll`, `swresample-5.dll` + `LICENSES/FFMPEG_LGPL.txt` + `EmbedCab="yes"`. `build.rs` no longer DELAYLOADs `mfplat`.

- `docs/licenses.md` + `LICENSES/FFMPEG_LGPL.txt` + `.github/workflows/ci.yml` LGPL gate: `Cargo.toml` static check + `Files.wxs` DLL check + `EmbedCab` + license presence + `lgpl` mention.

- Tests: 456 lib + 15 integration (tests/common produces only image fixtures via `image 0.25`; video fixtures are empty/corrupt/PNG-masqueraded `.mp4` files). No real mp4 fixture is committed. Bench `video_timeline` exists.

**User symptom:** valid H.264 mp4 opens (ftyp passes) but yields no frames → black screen / `ReachedEnd` immediately. "falta algo" is the expected stub behaviour.

## Affected Areas

- `src/core/video/ffmpeg.rs` — **primary change**: keep pure helpers untouched, replace `FfmpegDecoder` internals with real `ffmpeg_next` handles (`AVFormatContext`, `AVCodecContext` video+audio, `SwsContext`, `SwrContext`, stream indices, `AVRational` time bases, `Option<Duration>` duration, `AVFrame`/`AVPacket` reuse). Implement `open_inner` (avformat_open_input/find_stream_info/find_decoder/open2/sws_getContext/swr alloc), `next_sample` loop (av_read_frame → send_packet/recv_frame → sws_scale → ring.take_buffer → VideoFrame / swresample → f32 interleaved), `seek` (av_seek_frame BACKWARD + avcodec_flush_buffers), `refresh_duration` (AVFormatContext.duration / stream duration), `Drop` (free contexts). Must stay `!Send` confined.

- `src/core/video/backend.rs` — minimal: update doc comments removing MF references; no trait change.

- `src/core/video/player.rs` — minor: update `decoder_thread` comments (MF → FFmpeg), verify `audio_sample_rate` path (probe returns 0 → has_audio fallback already handles it), keep backpressure `ring.is_full() || audio.is_full()` (still correct for ffmpeg where video+audio decode on same thread).

- `Cargo.toml` — no feature default change (keep `default=[]`), ensure `ffmpeg-next 7` stays dynamic (no `static` feature). Possibly add `ffmpeg-next` feature unification docs; verify `hwaccel` still gates HW.

- `installer/windows/Files.wxs` / `Product.wxs` / `LICENSES/*` / `docs/licenses.md` — no code change, but verify DLL versions match `ffmpeg-next 7` expectations at build time; real DLLs must be present in `target/release` or `ReleaseDir` for WiX harvest (currently referenced but not on disk).

- `.github/workflows/ci.yml` — extend LGPL gate if needed; add `cargo test --features video` lane that installs FFmpeg dev libs (Windows: `winget`/`choco` ffmpeg shared; Linux: `libavcodec-dev` etc.) or keeps passing without them via `is_available` guard.

- `tests/common/mod.rs` + `tests/integration.rs` — add small real H.264 fixture generation (ffmpeg lavfi or checked-in tiny sample) for decode integration test gated behind `--features video` + `is_available`.

- `src/core/video/ring.rs`, `audio.rs`, `clock.rs`, `timeline.rs` — **not affected**; reuse as-is. Conversion `yuv_to_rgba` pure helper stays as bench baseline; real path uses `libswscale` via `ffmpeg_next::software::scaling`.

## Approaches

### Approach A — Keep `ffmpeg-next 7.x` dynamic and implement real `avformat/avcodec/sws/swr` in `ffmpeg.rs` (recommended)

- **Description:** Retain the current dependency (`ffmpeg-next = { version="7", optional=true }`) and feature wiring. Replace the stub body of `FfmpegDecoder` with idiomatic `ffmpeg_next` safe wrappers: `format::input(&path)`, `find_stream`, `codec::decoder::find`, `decoder.open()`, `software::scaling::Context` (`sws_scale` YUV420P/NV12→RGBA), `software::resampling::Context` (any → f32 interleaved at `audio_rate`). Keep all pure helpers (pts math, yuv fallback, probe, header validation as pre-check before avformat). Map `AVERROR` via `map_av_error`. Handle `AV_NOPTS_VALUE` fallback with existing `pts_to_duration`. `VideoInfo.width/height` from `codec.width/height` capped via `ring::output_size`; `duration` from `format.duration()` / `stream.duration` rescaled; `fps` from `stream.avg_frame_rate`; `has_audio` from stream discovery; `container_rotation` from displaymatrix side data if present.

- **Pros:**
  - Reuses 90% of the already-reviewed Slice1 surface: all pure helpers, `is_available` probe, header validation, HW backend types, LGPL bundling, CI gate, tests.
  - `ffmpeg-next` is the ecosystem standard (safe Rust wrappers, maintained, 7.x tracks FFmpeg 7.x that ships `avcodec-61.dll`); aligns with `Cargo.toml` MSRV 1.92 and existing `Cargo.lock`.
  - Dynamic LGPL compliance already solved (DLLs beside exe, `LICENSES/FFMPEG_LGPL.txt`, `EmbedCab`, user replacement rights). No static embedding.
  - Cross-platform: same crate resolves to `.so`/`.dylib` on Linux/macOS via pkg-config; `is_available` probe gracefully degrades to N-Edition / missing DLLs behaviour (already tested).
  - RAM/CPU priority preserved: `FrameRing` 6 + `AudioBuffer` 1.5 s remain; `sws_scale` and `swresample` are the fastest SW paths; resolution capped at 1920×1080 via `output_size`.
  - `Decoder` trait and `player::decoder_thread` protocol need zero changes; real decode is a drop-in.

- **Cons:**
  - FFmpeg dev libs must be on the builder for `--features video` (Windows vcpkg/ffmpeg shared build, Linux `apt libav*`), otherwise `cargo build --features video` fails at link time even though runtime is dynamic.
  - Must carefully manage unsafe lifetimes: `AVFrame`/`AVPacket` reuse, `Drop` order (`SwsContext`/`SwrContext` before `AVCodecContext` before `AVFormatContext`), and `!Send` confinement.
  - DLL version skew risk: `avcodec-61.dll` today, but FFmpeg 8 would want 62; probe already accepts 60/62 but `Files.wxs` hardcodes 61 — needs version bump discipline.
  - No committed real mp4 fixture yet → integration test needs synthetic generation or an external tiny sample.

- **Effort:** Medium. ~300-500 lines net in `ffmpeg.rs` + ~20 test lines + docs; `backend.rs`/`player.rs` comment-only. Split into `state.yaml` work units comfortably under 400-line review budget per slice.

### Approach B — Switch to `rsmpeg` (or raw `ffmpeg-sys-next` / `rume` bindings)

- **Description:** Replace `ffmpeg-next` with the lower-level `rsmpeg` crate (or direct `ffmpeg-sys-next` unsafe calls) that exposes raw `AV*` pointers with manual memory management. Would require hand-rolling reference counting, error mapping, and pixel format negotiation that `ffmpeg-next` already wraps.

- **Pros:**
  - Slightly smaller abstraction surface; direct access to `AVHWDeviceContext` for future HW accel without going through `ffmpeg-next` HW wrappers.
  - Alternative if `ffmpeg-next 7` maintenance lapses.

- **Cons:**
  - Loses the safe wrapper that justifies the current test coverage; essentially rewrites the same logic more unsafely.
  - Still LGPL dynamic with identical DLL bundling constraints — no compliance advantage.
  - Less ergonomic time-base/PTS helpers (would need to re-derive what `ffmpeg_next::Rational` already provides), more `unsafe` audit surface.
  - Ecosystem familiarity: team/archived change already invested in `ffmpeg-next` patterns; switching adds learning cost and churn.

- **Effort:** Medium-High. Similar scope to A plus wrapper rebuild.

### Approach C — Keep stub, require external player / OS decoder (no bundle)

- **Description:** Do not implement real decode; surface a UI message when `is_available()==false` suggesting the user install FFmpeg or use system player. Could shell out to `ffplay` or rely on Windows Media Foundation fallback.

- **Pros:**
  - Zero additional implementation or `unsafe`.
  - Keeps binary minimal and avoids LGPL bundling proliferation.

- **Cons:**
  - Directly contradicts the goal: H.264 mp4 would still render black / ReachedEnd. User reported "falta algo" — this approach keeps it missing.
  - MF fallback was deliberately removed (archived reason: weight, N-Edition fragility, patent exposure of OS codec). Reintroducing it reopens those debates.
  - Violates the "real decode renders frames" acceptance criterion.

- **Effort:** Low — but fails requirements.

## Recommendation

**Adopt Approach A.**

Rationale: the archived change already paid for 80% of the cost — feature gate, probe, header validation, HW types, LGPL bundling, installer, CI gate, pure helpers, and the entire pure playback stack. Approach A is the only path that turns the stub into real frames without moving any other boundary. It honours the user's explicit priority (RAM/CPU over binary size) by keeping `FrameRing`/`AudioBuffer` budgets and resolution cap, and it keeps `default=[]` so contributors without FFmpeg dev libs can still `cargo test` (only `cargo test --features video` requires libs). Approach B is a lateral move with more `unsafe`; Approach C does not satisfy the goal.

**Proposed sequencing for proposal/spec/design/tasks:**

1. `FfmpegDecoder` real open/close + duration/fps/has_audio reporting (no samples yet) — proves `avformat_open_input` + `find_stream_info` + header error mapping.
2. Video path `next_sample` → `SwsContext` → `FrameRing` (keep `yuv420p_to_rgba` as pure fallback bench oracle).
3. Audio path `next_sample` → `SwrContext` → `AudioBuffer` at `audio_rate`.
4. PTS/duration/seek polish (`pts_to_duration` reuse, `refresh_duration`, `av_seek_frame`+flush, `discard_before_seek_target` pre-roll).
5. Integration fixture + docs + CI `video` lane (optional dev-lib install) and DLL presence check for release.

Slice size: (1) and (3) individually under 400 lines; (2) is the largest — chain as two PRs if needed.

## Risks

- **DLL presence & version drift:** `Files.wxs` hardcodes `avcodec-61.dll` etc.; a `cargo update` to FFmpeg 8 would need 62. CI gate checks for substring presence, not exact version consistency with `Cargo.lock`. Mitigate by pinning `ffmpeg-next = "7"` and adding a `probe_dll` multi-version test already present; document replacement rights.
- **Build with `--features video` without dev libs fails:** Link-time failure even though runtime is dynamic, because `ffmpeg-sys-next` build script runs `pkg-config`/`vcpkg`. Mitigate by keeping `video` optional and documenting `cargo test` vs `cargo test --features video` in CONTRIBUTING/AGENTS.md; CI should not enable `video` on the default `cargo test` lane.
- **Unsafe lifetime & leak risk:** Forgotten `av_frame_free`/`av_packet_unref` leaks native memory close to the 64 MB managed budget. Require `Drop` impl and `ffmpeg_next` owned types (which already RAII-wrap) rather than raw sys pointers.
- **PTS/B-frame out-of-order delivery:** `pts_to_duration` handles `AV_NOPTS_VALUE` via `best_effort_timestamp`, but B-frames may still yield non-monotonic PTS; `timeline::frame_index_at` already tolerates it via `partition_point` non-panic short-circuit — keep that path.
- **Audio channel layout mismatch:** `AudioBuffer` is interleaved f32 stereo; `SwrContext` must be configured with target `AV_SAMPLE_FMT_FLT` + `channel_layout` matching `audio_rate` probe. A mismatch would sound as garbled/interleaved split. Verify with `cpal::default_output_config` probe.
- **SW scale perf on 1080p30:** ~8.29 MB per frame × 30 = 248 MB/s copy+s scale; within RAM budget but CPU-sensitive. The existing `yuv420p_to_rgba` bench baseline should be compared to `sws_scale` in `benches/video_timeline` to confirm no regression.
- **HW accel feature interaction:** `try_init_hw_device` today probes `is_available` then claims success as stub. Real HW path (Slice2) must call `av_hwdevice_ctx_create`; until then keep feature-gated `None` → SW fallback, already tested.
- **N-Edition Windows / CI headless:** `probe_dll` returns false in `cfg(test)` to avoid AV; real open must preserve that early Media error instead of attempting `avformat_open_input` on a non-existent loader state. The `SH_IMAGES_FFMPEG_STUB` atomic backdoor must remain for unit tests.
- **Fixture size / no-commit large binaries rule (AGENTS.md 8.2):** Real decode integration needs a tiny H.264 sample. Either generate via `ffmpeg -f lavfi` at test setup (requires `ffmpeg` on CI) or commit a <100 KB sample — needs ADR. For exploration, note the gap; spec phase will decide.
- **Binary size gate (<20 MB release):** Dynamic DLLs do not count toward `sh_images.exe` size (`lto/strip` profile) but do increase MSI. The 30 MB MSI gate mentioned in ADRs may need adjustment; spec should note the tradeoff the user already accepted (RAM/CPU > binary size).

## Affected Files Summary

| Path | Role in change |
|------|----------------|
| `src/core/video/ffmpeg.rs` | **core** — replace stub with real FFI |
| `src/core/video/backend.rs` | comment/doc alignment |
| `src/core/video/player.rs` | comment alignment, verify `audio_sample_rate_probe` zero-fallback |
| `Cargo.toml` | keep optional `video`, no static feature |
| `installer/windows/Files.wxs` & `Product.wxs` | verify DLL version match, already EmbedCab |
| `.github/workflows/ci.yml` | optional `video` lane with dev libs |
| `docs/licenses.md` / `LICENSES/FFMPEG_LGPL.txt` | already done, no change |
| `tests/common/mod.rs` & `tests/integration.rs` | synthetic real-mp4 fixture |

## Reusable Helpers (do not rewrite)

`pts_to_duration`, `av_rescale_q_saturating`, `yuv_to_rgba`, `yuv420p_to_rgba`, `probe_dll`/`is_available` (with `TEST_FFMPEG_STUB` atomics), `validate_file_header`, `map_av_error`, `check_codec_supported`, `preferred_hw_backend`/`hw_backend_name`/`select_hw_format`/`is_hw_format`/`try_init_hw_device*`/`hw_active`/`should_renegotiate_output`, plus entire `ring`/`audio`/`clock`/`timeline`/`seek`/`playback` pure stack.

## Ready for Proposal

**Yes.** Scope is bounded to `ffmpeg.rs` internals with no trait or protocol change; tradeoffs (LGPL dynamic, RAM/CPU priority, `video` feature optional, HW behind `hwaccel`) are settled from the archived change. Proposal should capture the sliced delivery (open → video → audio → PTS/seek polish → fixture/CI) and the 400-line review budget chaining.

## Next Recommended

`sdd-propose` for `ffmpeg-real-decode` — capture intent/scope/approach/rollback (rollback is simply "keep stub via `is_available()==false` path", already shipped).

---

## Appendix: artifact_store

- Openspec source of truth: `openspec/changes/ffmpeg-real-decode/explore.md` (this file).
- Optionally snapshot to Engram topic `sdd/ffmpeg-real-decode/explore` for session recovery (hybrid mode would do both; this session is `openspec`).
