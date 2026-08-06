# Changelog

All notable changes to Sh_Images will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
