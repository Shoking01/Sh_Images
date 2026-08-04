# Changelog

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