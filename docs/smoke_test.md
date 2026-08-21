# Smoke Test — FFmpeg Video Backend

## N-Edition (DLL absent) — REQ-FF-011

**Goal**: app launches and video UI degrades gracefully when FFmpeg DLLs are absent (Windows N).

Automated gate (runs on every `cargo test`):

```rust
// src/core/video/ffmpeg.rs
probe_nonexistent_dll_returns_false  // LoadLibraryW missing → false
open_missing_file_returns_media_not_panic // open without DLL → Media, no panic
```

Manual smoke:

1. Rename or remove `avcodec-61.dll` etc. from `target/release` / `C:\Program Files\Sh_Images`.
2. Or set env to simulate: no stub (`SH_IMAGES_FFMPEG_STUB` unset) on Windows N without Media Feature Pack.
3. Launch `sh_images.exe` — must open main window, no pre-main crash (DELayload removed).
4. Check `backend::is_available() == false` (tracing log `FFmpeg DLLs missing`).
5. UI: opening an MP4 should show `Media` error toast, not panic; image viewer still works.

Expected: MSI size 60-90MB with DLLs; without DLLs the exe is ~18MB and still launches.

## Cross-platform matrix — REQ-FF-001/002/003/007/008

| OS | Setup | Steps | Expected |
|----|-------|-------|----------|
| Windows 11 | FFmpeg DLLs bundled in MSI (EmbedCab yes) | Install MSI, open H.264 MP4 1080p, play, seek 30→10s, pause/resume, thumbnail (open_video_only) | Plays, pts monotonic, seek `<100ms` pts≥10s, drift `<50ms` over 60s, Ring ≤64MB, hw_active true if `hwaccel`+GPU else false fallback logged |
| Windows 11 N | DLLs removed / Media Feature Pack absent | Launch app, try play MP4 | `is_available false`, graceful Media error, no crash |
| Ubuntu 22.04 | `apt install ffmpeg` / libavcodec61 | `cargo run --features video -- video.mp4`, `cargo test --features video` | Same play/seek/thumbnail, swscale yuv→rgba, swresample 44.1→48kHz |
| macOS 14 | `brew install ffmpeg@7` | Same | Videotoolbox if hwaccel, else SW fallback |

Play/seek/thumbnail checklist (each OS):

- [ ] H.264 MP4 opens → `VideoInfo { has_audio true, fps Some(30), hw_active bool }`
- [ ] VP9/webm → `Media(unsupported codec)` (REQ-FF-001)
- [ ] Seek 30s→10s: `pts ≥10s` within 100ms, pre-roll discards `pts < target` until `≥target` (120ms coalesce)
- [ ] `refresh_duration` after open → `Some(duration)` or `None` (unknown) without panic
- [ ] `open_video_only` → `has_audio false`, no audio samples
- [ ] `set_output_size` 1080p→720p renegotiates, 1080p→1900x1070 does not (25% hysteresis)
- [ ] 60s drift check: `drift_ms(video_pts, clock) <50ms`
- [ ] `cargo bench video_timeline` SW ≤+25% vs MF baseline (criterion HTML), `capacity_bytes(1920,1080) ≤64MB`

## Running the gates

```powershell
cargo test --lib                    # 465+ tests, includes probe/bench gates
cargo bench --no-run                # benches compile
cargo bench video_timeline          # criterion, check +25% gate vs stored baseline
# MSI smoke (Windows, requires WiX)
$env:VERSION="0.2.4"; installer/windows/build.cmd
# Expect sh_images-0.2.4-x64.msi 60-90MB, dark -x shows avcodec-61.dll + LICENSES/FFMPEG_LGPL.txt
```

Rollback: `git revert` of `mf.rs` deletion restores Media Foundation; feature gate `--no-default-features` disables FFmpeg.
