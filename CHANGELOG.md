# Changelog

All notable changes to Sh_Images will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.4] - 2026-08-12

### Added

- **Video playback (Windows)** — MP4, M4V and MOV play in the viewer with audio,
  using Windows Media Foundation. No codecs are bundled: the system decoder is
  used, so the binary grows by only ~205 KB and no patent-encumbered decoder is
  redistributed. Linux and macOS build a stub that reports the format as
  unsupported.
- **Playback controls** — play/pause, ±5 s seek, volume up/down with a
  perceptual (quadratic) curve, mute, a seekable progress bar and an
  `MM:SS / MM:SS` position readout, in their own panel above the status bar.
- **Autoplay** — enabled by default; configurable via `video_autoplay` in
  `settings.toml`. Volume persists across sessions as `video_volume_percent`.
- **Video shortcuts** — `Space` play/pause, `Ctrl+→` / `Ctrl+←` seek ±5 s,
  `Ctrl+↑` / `Ctrl+↓` volume, `M` mute. All configurable in the shortcut dialog.
- **Slideshow awareness** — the slideshow waits for a playing video to finish
  instead of skipping past it after the interval.
- **MSRV job in CI** — `rust-version = "1.92"` was declared but never verified.
  CI now also runs `cargo test --release` and `cargo bench --no-run`.

### Changed

- **Texture uploads reuse the existing allocation** — video frames go through
  `TextureHandle::set_partial` instead of allocating a new texture per frame,
  avoiding ~30 texture and bind-group creations per second at 1080p30.
- **`MediaKind` separation** — thumbnail generation, neighbour preloading and
  EXIF reading now skip video files instead of spawning work that can only
  fail. `SUPPORTED_EXTENSIONS` is the union of the image and video lists, with a
  test that keeps them in sync.

### Fixed

- **`guess_format` no longer defaults to PNG** — saving to a path with a
  non-image extension wrote a PNG under the wrong extension without warning.
- **New shortcuts reach existing users** — `ShortcutMap::fill_missing_defaults`
  assigns defaults to actions added after a `settings.toml` was written;
  previously they stayed unbound until "reset all".
- **Duplicate default shortcuts are now caught** — `ShortcutMap::defaults()`
  collects without conflict detection, so two actions could silently share a
  binding. Covered by a new test.
- **Multi-track containers decode correctly** — `ReadSample` reports the *real*
  stream index, never the `MF_SOURCE_READER_FIRST_*_STREAM` pseudo-constants
  used to configure formats. Comparing the two classified every video frame as
  audio. Stream indices are now resolved by reading each stream's major type at
  open time, which also fixes files with several audio tracks (game recorders
  typically write game audio and microphone as separate tracks, with video not
  at index 0).
- **CI clippy on Linux/macOS** — `ramp_coefficient` and `AudioDevice.channels`
  are used only by the Windows audio backend; the unconditional import and field
  failed `cargo clippy -- -D warnings` on non-Windows targets, so both are now
  gated behind `#[cfg(windows)]`.

## [0.2.3] - 2026-08-08

### Added

- **Edit-navigation confirm dialog** — when navigating to another image with unsaved edits, a dialog prompts Save / Discard / Cancel so changes aren't lost silently.

### Fixed

- **Edit-mode navigation holes** — opening a file or selecting a sidebar thumbnail while editing now routes through the confirm dialog instead of bypassing it; slideshow is paused while editing and its toggle is suppressed in edit mode.

## [0.2.2] - 2026-08-06

### Fixed

- **App icon now shows in window title bar and taskbar** — icon is embedded
  into the `.exe` at build time and passed to eframe via `with_icon()`, so it
  renders correctly on Windows instead of showing the default system icon.
- **MSI installer: options dialog now appears** — fixed `<UIRef>` placement
  inside the `<UI>` block so the navigation override works; users now see the
  "Additional Options" screen with checkboxes for file associations and desktop
  shortcut.
- **MSI installer no longer accumulates duplicate entries** — `MajorUpgrade`
  now properly removes the previous version before installing the new one, so
  reinstalling no longer creates multiple entries in Add/Remove Programs.

### Added

- **MSI installer: optional features dialog** — lets users choose whether to
  associate image formats (`.png`, `.jpg`, `.jpeg`, `.bmp`, `.gif`, `.webp`,
  `.tif`, `.tiff`) and whether to create a desktop shortcut. Both default to
  enabled.

### Changed

- **Build pipeline** — `build.rs` now emits a Rust module with the icon's RGBA
  pixel data so the app can load the window icon at runtime.

## [0.2.1] - 2026-08-05

### Fixed

- Menu bar titles (File / View / Help) now respect the selected language.
- Windows-only imports and constants are gated behind `#[cfg(target_os)]` so
  cross-platform builds compile cleanly.

## [0.2.0] - 2026-08-05

### Added

- **Cursor-anchored zoom & pan** — zoom keeps the point under the cursor fixed;
  drag to pan when zoomed in.
- **Set as default image viewer** — registers `ShImages.ImageViewer` on Windows
  (HKCU) and opens the system default-apps settings page.
- **Built-in editor** — crop with a drag-on-viewer overlay; brightness, contrast,
  and saturation sliders (-100..=100); filters (grayscale, sepia, invert,
  black & white); save as PNG, JPEG, BMP, WebP, or TIFF.
- **English / Spanish UI** — all text routed through a `Lang` table; preference
  persisted in `settings.toml` (`language = "en"` or `"es"`). Selector under the
  gear menu.
- **Procedural toolbar icons** — clean, language-independent icons drawn at
  runtime with safe primitives (no `convex_polygon`).
- **RAM optimization** — adaptive image caching, preloading, and thumbnail
  generation tuned to available memory.

### Changed

- **Default language is English** — fresh installs now start in English.

### Fixed

- **MSI installer: embedded CAB** — `EmbedCab="yes"` ensures the `.cab` is
  packaged inside the `.msi`, eliminating the "source file not found cab1.cab"
  error when distributing the installer alone.
- **MSI installer: license** — replaced placeholder Lorem Ipsum with the actual
  MIT license agreement.
