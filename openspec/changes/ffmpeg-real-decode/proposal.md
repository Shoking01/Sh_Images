# Proposal: ffmpeg-real-decode

## Intent

Stub `next_sample()` returns `EndOfStream` — H.264 mp4 passes `ftyp` but 0 frames. Replace with real `ffmpeg-next 7` SW decode into `FrameRing`/`AudioBuffer`/`PlayClock` without changing `Decoder` trait.

## Scope

### In Scope
- `avformat`/`avcodec`/`sws`/`swr` in `ffmpeg.rs` reusing `pts_to_duration`, `sws_scale`, `swresample`, `validate_file_header`, `is_available`
- H.264/H.265 mp4/mov/mkv SW only; `FrameRing` 64 MB / `AudioBuffer` / `PlayClock` kept
- PTS fallback, duration/fps/has_audio, `av_seek_frame`+flush, `Drop`
- `video` optional (`default=[]`), DLLs + N-Edition kept

### Out of Scope
- VP9/webm, HW accel, fixtures >100 KB, MSI/shader YUV

### Non-Goals
- HW decode, new containers, size tuning

## Capabilities

### New Capabilities
- `video-decode`: H.264/H.265 mp4/mov/mkv — open/close, `next_sample` video→RGBA + audio→f32, PTS/duration/seek → `specs/video-decode/spec.md`

### Modified Capabilities
- None

## Approach

**A** (explore rec: keep `ffmpeg-next 7` dynamic):

- `format::input` + `find_stream_info` + `decoder::find`/`open` + `SwsContext`/`SwrContext`; keep `map_av_error`
- `next_sample`: `av_read_frame` → `send_packet`/`receive_frame` → `sws_scale` via `take_buffer` / `swresample` → f32
- `seek` `BACKWARD`+flush; `refresh_duration` from `AVFormatContext`; RAII `Drop`. No default/`static` change.

## Proposal question round

User chose **Continuar without correction**. `video` optional; H.264/H.265 first slice; missing DLLs → `Media` toast; SW only; RAM/CPU > size.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/core/video/ffmpeg.rs` | Modified | Real FFI loop |
| `src/core/video/backend.rs` | Modified | MF→FFmpeg docs |
| `src/core/video/player.rs` | Modified | Comments, audio fallback |
| `Cargo.toml` | Unchanged | Optional, no `static` |
| `installer/windows/Files.wxs` | Unchanged | Verify DLLs |
| `tests/common/mod.rs` | Modified | Fixture <100 KB |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| DLL drift 61→62 | Med | Pin `7`, probe 60/62 |
| `--features video` without libs | High | Keep optional |
| `AVFrame` leak 64 MB | Med | Owned types + `Drop` |
| B-frame PTS | Low | `pts_to_duration` |
| Swr layout | Low | `FLT` + layout |
| No DLLs on disk | High | `is_available` Media |

## Rollback Plan

Revert `ffmpeg.rs` to stub; `is_available==false` path ships. Delete >100 KB fixtures.

## Sliced Delivery (400-line budget)

| Slice | Content | Budget |
|-------|---------|--------|
| 1 | open/close + duration/fps/has_audio | <250 |
| 2 | Video `next_sample` → `SwsContext` → `FrameRing` | ~350 |
| 3 | Audio `next_sample` → `SwrContext` + PTS/seek + fixture/CI | <300 |

## Dependencies

- `ffmpeg-next 7` dynamic; dev libs only for `--features video`. WiX + `LICENSES/FFMPEG_LGPL.txt` bundled

## Success Criteria

- [ ] H.264 renders frames, duration/fps ok
- [ ] Audio resampled to `audio_rate` f32
- [ ] Seek keyframe-1+flush; no panic on corrupt/empty/PNG
- [ ] `cargo test` without `video` passes; with `video` passes
- [ ] `clippy -D warnings` + LGPL gate green
