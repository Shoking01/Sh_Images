# Licenses

## Application

Sh_Images is MIT (see `License.rtf` in the installer and `LICENSE` at the repo root).

## FFmpeg (LGPL-2.1, dynamic)

Video decoding on all platforms uses **FFmpeg 7.x** via the `ffmpeg-next` crate
with **dynamic LGPL** linking.

- **Linking mode**: dynamic only. `Cargo.toml` declares
  `ffmpeg-next = { version = "7", optional = true }` with no `static` feature.
  The `ffmpeg-sys-next` build never links `libavcodec.a` into the binary; at
  runtime the loader resolves `avcodec-61.dll` / `libavcodec.so.61` etc. via
  `LoadLibraryW` probe in `src/core/video/backend.rs:is_available()`.
- **Bundled binaries**: the WiX installer (`installer/windows/Files.wxs`,
  `Product.wxs` with `EmbedCab="yes"`) ships the FFmpeg shared libraries
  (`avcodec-61.dll`, `avformat-61.dll`, `avutil-59.dll`, `swscale-8.dll`,
  `swresample-5.dll`) alongside `sh_images.exe`. On Linux/macOS the same
  `.so`/`.dylib` are resolved from the system or app bundle — never statically
  embedded.
- **Attribution file**: `LICENSES/FFMPEG_LGPL.txt` is included in the MSI and
  documents the LGPL-2.1 source location (https://ffmpeg.org, LGPL text at
  https://www.gnu.org/licenses/old-licenses/lgpl-2.1.txt) and user-replacement
  rights.
- **N-Edition behavior**: if FFmpeg DLLs are absent (Windows N), `is_available()`
  returns `false` and video features gracefully disable — no crash, no import
  load — verified by unit test `probe_nonexistent_dll_returns_false`.
- **CI gate**: `.github/workflows/ci.yml` enforces LGPL compliance — it fails
  the build if `Cargo.toml` enables `ffmpeg-next/static`, if `Files.wxs` does
  not reference the DLLs, or if `LICENSES/FFMPEG_LGPL.txt` is missing.

### Building with the `video` feature

End users never need FFmpeg SDKs (the MSI ships the runtime DLLs), but
building or testing `--features video` requires FFmpeg **development
libraries** + `pkg-config` (`libavcodec-dev libavformat-dev libavutil-dev
libswscale-dev libswresample-dev` on Debian/Ubuntu; Homebrew `ffmpeg` on
macOS). CI runs this lane as the `video-decode` job in `.github/workflows/ci.yml`.

### Replacing FFmpeg

Because linking is dynamic, you may replace the bundled DLLs/so/dylibs with
your own compatible LGPL build. The app probes `avcodec-61.dll` (also accepts
`60`/`62` for forward compat) at startup; replacement requires matching major
version.

### Verifying the bundle

```powershell
# Check MSI contains FFmpeg DLLs and license
# (requires WiX dark or 7-Zip)
dark.exe -x out installer/windows/sh_images-*.msi
dir out\LICENSES\FFMPEG_LGPL.txt
dir out\avcodec-*.dll
```
