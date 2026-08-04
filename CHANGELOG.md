# Changelog

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