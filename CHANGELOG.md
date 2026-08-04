# Changelog

## Fase 4 — Metadatos EXIF

- Lectura de metadatos EXIF (JPEG/TIFF) en `core/exif.rs` con `kamadak-exif`.
- Panel derecho de información (`ui::info_panel`) con campos curados
  (Fabricante, Modelo, Fecha, ISO, Apertura, Obturador, Focal, Orientación).
- Acción `ToggleInfo` (atajo `I`) para mostrar/ocultar el panel.
- Carga asíncrona de EXIF en segundo plano con cache por path.