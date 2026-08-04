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

## ADR-007: Centralización de acciones vía enum `Action`

- **Contexto:** En la Fase 3, toolbar (mouse), menú y atajos de teclado
  (teclado) deben disparar los mismos efectos sobre la app. Tener cada path
  llamando a `open_dialog()`, `navigate()`, etc. directamente produce N copias
  de la lógica, dispersión de `tracing::info!` de auditoría y difficultad para
  extender/remapear acciones.
- **Decisión:** Un enum `core::actions::Action` con 10 variantes
  (`Open`, `Prev`, `Next`, `RotateCw`, `RotateCcw`, `Fit`, `Fullscreen`,
  `ToggleTheme`, `ToggleSidebar`, `EditShortcuts`) y un único método
  `dispatch(Action)` en `app.rs`. Las tres fuentes (toolbar::show devuelve
  `Option<Action>`; el menú lo invoca directamente; `handle_shortcuts` mapea
  la tecla al `Action` vía `ShortcutMap::action_for`) reducen a un único
  punto de efecto. Cada variante conoce su `label()` y su `default_shortcut()`,
  y `Action::all()` las enumera en orden estable para la UI del editor.
- **Consecuencias:** La lógica de un efecto ("abrir dialogo", "toggle tema")
  vive una sola vez, en `dispatch`. Añadir una nueva acción (e.g. `Slideshow`)
  es: añadir variante a `Action`, un `match` arm en `dispatch`, una entrada en
  `default_shortcut()` opcional. El testing se simplifica: `Action` es puro y
  testeable; `dispatch` solo se cubre con integration tests de la app. El menú
  y toolbar pueden iterar sobre `Action::all()` sin acoplarse a un catálogo
  separado.
- **Alternativas:** (a) Devolver callbacks/closures desde cada fuente de
  acción (cierres opacos, no inspeccionables ni serializables), (b) un string
  `&'static str` por acción (no type-safe, errores en runtime), (c) mantener
  la lógica dispersa y aceptar la duplicación (acoplamiento N×N de las fuentes
  a los efectos).

## ADR-008: Rotación visual vía mesh con UVs permutados (sin re-decodificar)

- **Contexto:** La Fase 3 añade rotación 90° CW/CCW sobre la imagen actual.
  Rotar la textura en GPU es trivial (`egui::Mesh` con UVs permutadas); rotar
  re-descodificando el JPEG/PNG en memoria es costoso y no aporta (la imagen
  en disco no cambia, solo la presentación). El `Viewer` ya pintaba con
  `ui.painter().image(...)` (un solo quad, UV `(0,0)..(1,1)`).
- **Decisión:** Añadir `rotation: u8` (0..=3) y `effective_size()` (intercambia
  dimensiones si `rotation` es par/impar) a `ViewTransform`. La rotación
  re-aplica `fit()` (resetea zoom y pan, evita que una imagen rotada salga
  del canvas). `ViewTransform::rotated_uv(corner, rotation)` mapea cada esquina
  TL/TR/BR/BL al UV permutado. El viewer decide: si `rotation == 0`, usa el
  `painter.image` (camino barato); en otro caso, construye un `egui::Mesh`
  con 4 vértices y 6 índices, asigna los UVs permutados a las esquinas y lo
  añade al painter con `Shape::mesh`.
- **Consecuencias:** Cero coste de decodificación: la imagen rotada se renderiza
  desde la misma textura GPU. Los tests de math (`fit_zoom_swaps_dimensions_on_odd_rotation`,
  `rotated_uv_permutes_corners`, snapshot `snapshot_rotation_math`) verifican
  la corrección sobre imágenes NO cuadradas (viewport 1000×500 con imagen
  1000×500: fit cambia de 1.0 a 0.5 al rotar 90°). El viewer paga un mesh de
  4 vértices solo cuando hay rotación; el camino sin rotación sigue siendo el
  `painter.image` de Fase 1. Filtros/operaciones que necesiten la imagen
  rotada como bitmap (exportar a PNG rotado) no están cubiertos por esta
  decisión; eso sería Fase X.
- **Alternativas:** (a) Re-decodificar la imagen y aplicar `image::imageops::rotate90`
  (coste lineal en pixels; latencia de 100–500 ms para una imagen 4K — viola
  AGENTS.md §6.2 de "tiempo de rotación < 50 ms"), (b) mantener N texturas
  pre-rotadas en GPU (desperdicia memoria, N=4 copias), (c) implementar la
  rotación solo en math de viewer (`Painter::with_clip_rect` + `rotate` shadres)
  más complejo y propenso a errores de muestreo en los bordes.

## ADR-009: Metadatos EXIF con `kamadak-exif` y panel de información

- **Contexto:** Fase 4 necesita leer EXIF (JPEG/TIFF) y mostrarlo; `core/exif.rs`
  era un stub. La lectura es I/O y no debe bloquear el UI thread (AGENTS.md §7.1).
- **Decisión:** `kamadak-exif` 0.6.1 (BSD-2-Clause, estándar, Plan.md §6) en
  `core/exif.rs` con un modelo curado `ExifImage` (Make, Model, fecha, ISO,
  f-number, ExposureTime, FocalLength, Orientation) y `Rational`. Un worker de
  `app.rs` lee el EXIF en segundo plano y lo cachea por path (`ExifRead`
  distingue Found/None/Error). Un nuevo `ui::info_panel` pinta un `Panel::right`
  con las filas formateadas; `Action::ToggleInfo` (shortcut `I`) lo alterna.
- **Consecuencias:** La UI no bloquea al leer EXIF grande; el cache por path
  evita re-parseos; imágenes sin EXIF muestran "Sin metadatos"; errores → toast.
  Un JPEG/TIFF sin bloque APP1 hace que la librería devuelva `Error::NotFound`,
  que se mapea a `Ok(None)` (no es un error).
- **Alternativas:** `exif` (menos mantenido), parser manual del APP1/TIFF
  (frágil), lectura síncrona en el UI thread (viola §7.1).
