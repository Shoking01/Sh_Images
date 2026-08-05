# Changelog

## [0.2.0] - 2026-08-05

### Added

- **Cursor-anchored zoom and image panning.** Zooming now keeps the image
  point under the cursor fixed (`ViewTransform::apply_zoom_at`) and a new
  drag-to-pan gesture lets you move the image around when zoomed in. A clip
  rect keeps the image from spilling over the menu and toolbar.
- **Set as default image viewer (Windows).** New toolbar action walks the
  user through a confirmation dialog before registering `ShImages.ImageViewer`
  under `HKCU\Software\Classes` and opening the system's "default apps"
  settings page.
- **Built-in image editor.** Right-side panel with crop (drag to select on the
  viewer, live overlay, "Apply crop" mutates the in-memory image), color
  adjustments (brightness, contrast, saturation, -100..=100) and filters
  (grayscale, sepia, invert, black & white). Edits can be saved to disk via
  the native file dialog as PNG / JPEG / BMP / WebP / TIFF, suffixed with
  `_edit` by default.
- **English / Spanish UI.** All visible strings routed through a new
  `Language` enum and a static `Translations` table; preference persisted to
  `settings.toml` as `language = "es"` or `language = "en"`. Selector lives
  under the gear menu (⚙ → 🌐 Idioma / Language).
- **Unicode toolbar icons** (◀ ▶ 🔄 ⊡ ⛶ ☰ ℹ ⏵ ✎ ⚙ ★) replace the previous
  text labels and stay readable across both languages.
- **Compact gear menu.** Theme toggle, keyboard shortcuts, default-viewer
  action and language selector collapsed under ⚙ so the toolbar stays
  uncluttered.

### Changed

- Default UI language switched from Spanish to English
  (`Settings::default_language` and `Settings::default` both return
  `Language::En`); existing Spanish users keep their preference from
  `settings.toml`.
- Code comments translated to English (or removed when redundant).

### Internal

- New modules: `core::lang`, `core::edit_state`, `core::editor`,
  `ui::editor`, `utils::default_app`.
- New actions: `SetLangEs`, `SetLangEn`, `SetDefaultViewer`, `Edit`,
  `SaveCopy`, crop workflow helpers.
- Tests: 200 unit + 3 main + 9 integration (all passing).

## Fase 5 — Windows Packaging (2026-08-04)

- Installer MSI (WiX 3.14, perMachine/x64) con asociaciones de archivos para
  PNG, JPEG, BMP, GIF, WebP y TIFF.
- CLI: `sh_images.exe <path>` abre la imagen directamente (sin diálogo).
- Icono multi-resolución (16–256px) generado desde `assets/icon.svg` en
  `build.rs` y embebido en el `.exe` (windowed, sin consola).
- Workflow de release en GitHub Actions (`release.yml`): triggers en tags `v*`
  y `workflow_dispatch`, build release, generación de MSI, validación de size
  (<30MB) y upload como artifact.
- `[profile.release]` con `lto=true`, `strip=true`, `codegen-units=1` (20.9→15.9MB).
- ADR-012 documenta la decisión de WiX + build.rs icon + CLI path.

## Fase 4 — Metadatos EXIF, GIF animado y slideshow

- Lectura de metadatos EXIF (JPEG/TIFF) en `core/exif.rs` con `kamadak-exif`.
- Panel derecho de información (`ui::info_panel`) con campos curados
  (Fabricante, Modelo, Fecha, ISO, Apertura, Obturador, Focal, Orientación).
- Acción `ToggleInfo` (atajo `I`) para mostrar/ocultar el panel.
- Carga asíncrona de EXIF en segundo plano con cache por path.
- Reproducción de GIF animado en bucle (`LoadedImage::Animated`, retardos por
  frame, textura reconstruida al cambiar de frame) con límites de decodificación
  (`set_limits`) que evitan un hang en inputs corruptos.
- Slideshow automático: `ToggleSlideshow` (F5), `SlideshowFaster` (","),
  `SlideshowSlower` ("."); intervalo persistido en settings (default 5 s) con
  límites 1–60 s; pausa al navegar o hacer zoom.