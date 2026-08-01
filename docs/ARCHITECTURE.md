# Arquitectura — Sh_Images

## ADR-001: GUI con `eframe` + `egui`

- **Contexto:** Necesitamos una GUI nativa, ligera y sin runtime externo
  (no Electron/WebView).
- **Decisión:** `eframe` (winit + wgpu; glow opcional) con `egui` (immediate-mode).
- **Consecuencias:** Rendimiento excelente para 2D, sin árbol de widgets
  persistente; hay que reconstruir la UI cada frame.
- **Alternativas:** `iced` (funcional, elíptico), `gtk-rs` (pesado, dependencias
  de sistema), `tauri` (usa WebView).

## ADR-002: Error centralizado `ShImagesError`

- **Contexto:** Múltiples módulos (I/O, decode, config) producen errores.
- **Decisión:** Un único enum `ShImagesError` con `thiserror`, alias
  `Result<T>`.
- **Consecuencias:** La UI maneja un solo tipo de fallo; `#[from]` convierte
  `std::io::Error` automáticamente; errores de `image` se mapean a `Decode`/
  `UnsupportedFormat` para no acoplar el crate. Las variantes `Config`/`Decode`/
  `UnsupportedFormat` llevan `String` como mensaje de contexto (patrón thiserror
  convencional); la identidad de la variante es lo que el código hace `match`,
  no la cadena.
- **Alternativas:** Error por módulo con `Into<ShImagesError>` (más boilerplate
  en Fase 0), `anyhow` (pierde tipado).

## ADR-003: Decodificación con `image` crate

- **Contexto:** Soporte amplio de formatos (PNG, JPEG, GIF, BMP, WebP, TIFF,
  AVIF).
- **Decisión:** Delegar a `image` 0.25.
- **Consecuencias:** Formatos y fixes de seguridad vienen del ecosistema;
  el mapeo de errores queda en `core::image_loader`.
- **Alternativas:** `zune-image` (menos maduro), bindings a libvips
  (complejidad C).

## ADR-004: Target de librería + binario

- **Contexto:** Los stubs de módulos no usados disparan `dead_code` en un crate
  binario con `-D warnings`; los tests de integración y benchmarks necesitan
  importar la lógica.
- **Decisión:** `src/lib.rs` (toda la lógica) + `src/main.rs` (wrapper fino).
- **Consecuencias:** Los benchmarks (`benches/`) importan `sh_images::*`;
  los tests de integración futuros (`tests/`) podrán importar la lógica
  sin acoplar a la UI; `main.rs` queda ≤ 50 líneas.

## ADR-005: Dependencias en sus versiones estables actuales

- **Contexto:** El plan original fijaba `eframe`/`egui` 0.31 y `toml` 0.8
  (línea conocida de 2025).
- **Decisión:** Usar `eframe`/`egui` 0.35 y `toml` 1.x (líneas estables
  actuales, 2026), y commitear `Cargo.lock`.
- **Consecuencias:** La API de eframe 0.35 cambió (`App::ui` en vez de
  `App::update`; `CentralPanel::show` con `&mut Ui`); MSRV sube a Rust 1.92.
  El código del proyecto quedó adaptado a esa API.
- **Alternativas:** Mantener 0.31 (acumular deuda de migración), subir más
  tarde (migración más costosa con más código).

## ADR-006: Pipeline de miniaturas separado del full-res (sidebar)

- **Contexto:** El sidebar de miniaturas necesita decodificar N imágenes de una
  carpeta sin bloquear el UI thread ni competir con el pipeline de full-res del
  S2 (decodificación bajo demanda + cache LRU + pre-carga N±1). Decodificar en
  el UI thread congela la app; reutilizar el canal del S2 mezclaría latencia de
  thumbnails con la imagen en pantalla.
- **Decisión:** Un pipeline dedicado con (1) cola FIFO `ThumbQueue`
  (`Mutex` + `Condvar`, en `core/thumb_queue.rs`), (2) pool acotado de 3
  workers que decodifican a `THUMB_MAX` (96px) y (3) un `ThumbnailCache` en
  memoria sin evicción (`core/thumbnail_cache.rs`), separado del LRU del S2.
  La UI no recibe las imágenes por el canal: solo recibe una notificación
  ligera (`mpsc`) que dispara repaint y lee el cache directamente. Los workers
  llaman `ctx.request_repaint()` para despertar la UI inactiva (mismo patrón
  que `spawn_load` del S2).
- **Invalidación de carpeta:** Al abrir una carpeta (`open_path`) se (a)
  incrementa un contador de generación `thumb_epoch` (`AtomicU64`), (b) se
  drena la cola con `ThumbQueue::drain`, (c) se limpia el cache y las texturas.
  Los workers capturan el epoch al extraer el path y lo re-verifican tras la
  decodificación: si cambió, descartan el resultado. El `drain` es seguro
  porque los workers liberan el lock mientras esperan (`Condvar`), a diferencia
  de compartir un `Arc<Mutex<Receiver>>` (que mantendría el guard durante un
  `recv()` bloqueante y deadlockearía con el drenado).
- **Consecuencias:** Las miniaturas se generan en paralelo sin bloquear la UI;
  al cambiar de carpeta, los decodes en vuelo se descartan y los paths en cola
  se drenan (sin re-decodificar carpetas anteriores). `ThumbnailCache` sin
  evicción es aceptable en Fase 2 ("decenas a cientos" de imágenes, ~37 KB por
  miniatura); se revisará para carpetas muy grandes (virtualización/LRU de
  miniaturas). Un path ya extraído por un worker y en decodificación no puede
  cancelarse: su resultado se descarta por el epoch, no por el drain.
- **Alternativas:** (a) Compartir el canal del S2 (mezcla de prioridades,
  decodificaciones full-res desperdiciadas), (b) decodificar thumbnails en el
  UI thread (freeze perceptible), (c) drenar un `Arc<Mutex<Receiver>>`
  compartido (deadlock: el guard se mantiene durante `recv()` bloqueante),
  (d) un worker que decodifica en serie (lento para carpetas grandes).
