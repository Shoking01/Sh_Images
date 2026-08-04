# Fase 4 — GIF animado y slideshow automático — Design

> Fecha: 2026-08-03 · Estado: Aprobado por usuario · Alcance: sub-features 3 y 4
> de la Fase 4 de Plan.md, para completar la Fase 4.

## 1. Contexto

Sh_Images tiene completas las Fases 0-3 y la parte EXIF de la Fase 4 (lectura
de metadatos EXIF + panel de información, ADR-009). El visor decodifica imágenes
en un pool de workers, las cachea en un LRU por path (`ImageCache` guarda
`DynamicImage`), construye una textura de egui por imagen y la pinta con
`ViewTransform` (que soporta rotación vía mesh). El sidebar genera miniaturas a
96px a partir de la misma `DynamicImage`.

Quedan dos sub-features de la Fase 4 (Plan.md §4):

1. **Soporte para GIF animado** — hasta hoy `image::open` decodifica solo el
   primer frame de un GIF; queremos reproducir la animación respetando los
   retardos por frame.
2. **Slideshow automático** — avanzar por la carpeta cada N segundos con
   pausa al interactuar y velocidad configurable.

## 2. Decisiones de diseño

### 2.1 Representación unificada `LoadedImage` (enfoque A, aprobado)

**Contexto:** el pipeline actual guarda `DynamicImage` en el `ImageCache`
(una imagen por path). Un GIF animado es N frames; el cache necesita saber
que hay varios frames para contabilizar memoria y que la app pueda seleccionar
el frame activo.

**Decisión:** un enum en `core/image_loader.rs`:

```rust
pub struct AnimatedFrame { pub image: DynamicImage, pub delay: Duration }
pub struct AnimatedImage  { pub frames: Vec<AnimatedFrame>, pub total_duration: Duration }
pub enum LoadedImage {
    Static(DynamicImage),
    Animated(AnimatedImage),
}
```

`load_image(path) -> Result<LoadedImage>`. `ImageCache` guarda `LoadedImage` en
vez de `DynamicImage`. Los callers que solo necesitan "la imagen" usan
`LoadedImage::first_frame()`/`dimensions()`/`frame_at(elapsed)`.

**Alternativas consideradas:** (a) cache animado separado (dos pipelines, más
dispersión en preload/apply/thumbnails — descartado), (b) decodificar frames
bajo demanda en el UI thread (rompe el modelo de worker + cache, latencia y
bloqueo del UI — descartado, AGENTS.md §7.1).

### 2.2 Decodificación de frames con `GifDecoder`

**Decisión:** en `load_image`, usar `image::ImageReader::open(path)?
.with_guessed_format()?` para detectar el formato por magic bytes. Si
`format() == Some(ImageFormat::Gif)`, construir `GifDecoder::new(BufReader)`
(implementa `AnimationDecoder`), iterar `into_frames()` recolectando cada
`Frame` con `into_buffer() -> RgbaImage` y `Duration::from(frame.delay())`.
`total_duration` se precomputa (suma de retardos) y se usa para el wrap-around
del bucle. Si el resultado tiene **un solo frame**, devolver `Static` (no hay
nada que animar). Cualquier frame con retardo 0 se clamp a un mínimo de 20 ms
para evitar busy-loop del repaint. GIF corrupto → `ShImagesError::Decode`
(nunca panic).

API verificada en `image` 0.25.10: `AnimationDecoder<'a>` con
`into_frames(self) -> Frames` y `loop_count()`; `Frame` con
`delay() -> Delay` (con `impl From<Delay> for Duration`), `into_buffer()`,
`buffer()`, `buffer_mut()`. `GifDecoder<R: BufRead + Seek>` implementa
`AnimationDecoder`.

### 2.3 Selección de frame activo: `frame_at(elapsed)` (puro)

**Decisión:** `LoadedImage::frame_at(elapsed: Duration) -> &DynamicImage`:

- `Static(img)` → `img`.
- `Animated(anim)` → `t = elapsed.as_millis() % total_duration.as_millis()`
  (total > 0 garantizado tras el clamp de 20 ms) y búsqueda acumulada por
  los retardos; `if let Some(last)` para el final (sin `unwrap`, AGENTS.md §2.1).

Esto vive en `core/` y es 100% testeable (wrap, retardos desiguales, 1 frame).

### 2.4 Render en app: `AnimState` + textura del frame activo

**Decisión:** `app.rs` guarda `anim: Option<AnimState>` con
`AnimState { started: Instant, current_frame: usize }`.

- En `apply_decoded`, al leer `LoadedImage::Animated` se fija
  `anim = Some(AnimState::new())` y se construye la textura del frame 0; al
  leer `Static`, `anim = None`.
- `tick_animation()` en cada frame: si `anim` es `Some` y la imagen actual es
  animada → `let dt = started.elapsed(); let f = image.frame_at(dt);` si el
  puntero frame cambió → `make_texture(f)` (reconstrucción, se reemplaza
  `self.texture`) y `current_frame = idx`. Luego
  `ctx.request_repaint_after(próximo cambio de frame)` para despertar la UI
  aunque esté idle.

El viewer no cambia (pinta una sola textura; la rotación mesh se aplica a
cualquier frame). Zoom/fit no cambian. `thumbnail_gen` usa
`LoadedImage::first_frame()` (miniaturas = primer frame del GIF).

### 2.5 Slideshow puro en `core/slideshow.rs` (nuevo)

**Decisión:** módulo `core` nuevo, sin dependencias de UI, con funciones
puras:

```rust
pub fn default_interval() -> Duration;     // 5 s
pub fn faster(interval: Duration) -> Duration;  // max(1 s, interval / 2)
pub fn slower(interval: Duration) -> Duration;  // min(60 s, interval * 2)
pub fn elapsed_reached(elapsed, interval) -> bool;
```

Límites [1 s .. 60 s]. Testeable al 100%.

### 3.6 Acciones y atajos

**Decisión:** añadir a `Action` (y sus helpers `label()` / `default_shortcut()` /
`all()`):

- `Action::ToggleSlideshow` — label "Iniciar/detener slideshow", default `F5`.
- `Action::SlideshowFaster` — label "Slideshow más rápido", default ",".
- `Action::SlideshowSlower` — label "Slideshow más lento", default ".".

Nuevos `KeyCode::{ KeyF5, Comma, Period }` + `to_str` / `from_str` +
traducción en `ui/shortcut_dialog.rs` (`egui::Key::F5`, `Comma`, `Period`) +
actualizar snapshots de atajos. `Action::all()` pasa de 11 a 14. Botón "▶" en
la toolbar.

### 3.7 Configuración del intervalo

**Decisión:** campo `slideshow_interval_secs: u64 = 5` en `Settings` con
`#[serde(default)]` (la migración de settings viejos sin el campo deserializa
al default; ADR-005-grade). Se persiste con `Settings::save` igual que el tema
y los atajos.

### 4.1 Estados del slideshow en `app.rs`

- Campos: `slideshow_active: bool`, `slideshow_interval: Duration`
  (inicializado desde `settings.slideshow_interval_secs`).
- `dispatch`:
  - `ToggleSlideshow` → alterna `slideshow_active` + reinicia el timer +
    `request_repaint`.
  - `SlideshowFaster`/`SlideshowSlower` → `faster`/`slower` sobre el intervalo,
    persisten a `settings`, reinician el timer.
- `ui()`: si activo → si `elapsed >= interval` avanzar una imagen via método
  interno `advance_slideshow()` (no pasa por dispatch y NO se pausa a sí mismo)
  + reset del timer; `ctx.request_repaint_after(interval)` para despertar idle.
- **Pausa al interactuar:** `navigate()` (←/→ teclado/botón), `navigate_to`
  (miniatura) y el zoom (`resp.zoomed`) apagan `slideshow_active`.
- Indicador visual "▶" en la toolbar derecha (junto al "Tema"/fullscreen)
  cuando el slideshow está activo.

## 4. Data flow

```
open/navigate → load_image(path) → LoadedImage
   ├─ Static(img)      → texture = make_texture(img) ; anim = None
   └─ Animated(anim)   → texture = make_texture(frame_at(0)); anim = Some(started=now, 0)
frame n (anim activo):  tick_animation() → frame_at(elapsed) → si cambió → make_texture + request_repaint_after(resto)
slideshow:              ui loop → elapsed>=interval → advance_slideshow (no pausa) → navigate interno
interacción manual:     navigate/navigate_to/zoomed → slideshow_active = false
```

## 5. Error handling

- `load_image` devuelve `Result<LoadedImage>`; errores de `image` se mapean
  como hoy (`Io` / `UnsupportedFormat` / `Decode`). Un GIF corrupto → `Decode`,
  nunca crash; el toast existente se muestra.
- `frame_at` nunca has `unwrap`/`expect`: indexación con `if let Some(last)`
  como fallback pese a `frames` no vacío (invariante garantizada en el loader).
- El worker de load ya encapsula el error en el canal; ningún cambio de flujo
  de errores para los formatos estáticos.

## 6. Testing

### 6.1 Unitarios `core/image_loader` (≥ 90%)

- PNG/JPEG → `Static` con dimensiones correctas.
- GIF animado (generado en el test con `GifEncoder`) → `Animated` con N frames
  y los retardos esperados (incluido clamp de un delay 0 → 20 ms).
- GIF de 1 frame → `Static` (no anima).
- GIF corrupto → `Err(ShImagesError::Decode(_))`.
- Archivo inexistente → `Err(Io)`, extensión desconocida → `Err(Unsupported)`.

### 6.2 Unitarios `frame_at` y `core/slideshow`

- `frame_at`: frame activo por tiempo acumulado; wrap-around al
  `total_duration` (bucle infinito); `frame_at` sobre el frame en `Static`.
- `default_interval` == 5 s; `faster` y `slower` respetan [1, 60]; el borde
  floor/ceiling no desborda.

### 6.3 Unitarios `core/image_cache`

- Insertar un `LoadedImage::Animated` cuenta memoria = suma de frames;
  evicción LRU normal (tamaños sumados); `get` por path devuelve la misma
  representación.

### 6.4 Unitarios `settings`

- default `slideshow_interval_secs == 5`; round-trip TOML; migración: un
  `settings.toml` sin el campo deserializa con 5 (no error).

### 6.5 Integración `tests/integration.rs`

- Flujo 8 — GIF: generar un GIF animado de 2 frames con `GifEncoder`, abrirlo
  con `load_image` → `Animated` con N frames y delays; `frame_at` devuelve el
  frame correcto; un GIF corrupto no crashea (`Decode`).
- Flujo 9 — Slideshow: `faster`/`slower` clampa a [1 s, 60 s]; el intervalo
  por defecto es 5 s; `settings` roundtrip del intervalo (con `serde(default)`).

### 6.6 Cobertura

- `core/image_loader` ≥ 90%, `core/slideshow` ≥ 90%, `config` ≥ 85% (código
  nuevo), `ui` sólo se extiende con `Action`/`KeyCode` puros.

## 7. Snapshots

- Se regenera el snapshot defaults de `shortcuts` (nuevos `F5`, `,`, `.`).
- Se revisan los snap` del `KeyBinding` antes de commitear (nunca aceptar a
  ciegas, AGENTS.md §7).

## 8. Files afectados

```
src/core/image_loader.rs   MODIFICAR — LoadedImage, AnimatedImage, load_image(frames GIF)
src/core/image_cache.rs     MODIFICAR — guarda LoadedImage, estimate_bytes multi-frame
src/core/slideshow.rs       CREAR — default/faster/slower/elapsed_reached (puro)
src/core/actions.rs         MODIFICAR — ToggleSlideshow, SlideshowFaster, SlideshowSlower
src/core/shortcuts.rs       MODIFICAR — KeyCode F5/Comma/Period + to_str/from_str
src/ui/shortcut_dialog.rs   MODIFICAR — mapeos egui F5/Comma/Period
src/ui/toolbar.rs           MODIFICAR — botón "▶"
src/config/settings.rs       MODIFICAR — slideshow_interval_secs
src/app.rs                  MODIFICAR — anim state, tick_animation, slideshow state
src/ui/mod.rs               AÑADIR core/slideshow (si se expone) — sin cambio else
benches/opening.rs          MODIFICAR — adaptar a LoadedImage (camino Static)
src/core/thumbnail_gen.rs    MODIFICAR — first_frame() para miniaturas
tests/integration.rs         MODIFICAR — flujos 8-9
tests/common/mod.rs          MODIFICAR — helper make_animated_gif
docs/ARCHITECTURE.md         MODIFICAR — ADR-010 (Load/Anim) y ADR-011 (slideshow)
CHANGELOG.md                 MODIFICAR — entrada Fase 4 (GIF + slideshow)
Plan.md                      MODIFICAR — marcar GIF + slideshow de Fase 4 como hechos
```

## 9. Riesgos

| Riesgo | Mitigación |
|--------|-----------|
| GIFs grandes con muchos frames consumen mucha RAM (suma de frames en LRU) | El LRU ya limita la memoria y los frames se contabilizan; el clamp de delay evita repaint floods |
| `Frame::delay()` = 0 → repaint busy-loop | Clamp mínimo 20 ms en el loader |
| Rotación mesh con frames generados sobre cada frame | El viewer no cambia; solo se reconstruye la textura al cambiar de frame |
| El slideshow y la navegación manual se pisotean | `advance_slideshow()` interno no pasa por dispatch y no se pausa; la interacción manual pausa |
| Agregar 3 acciones infla `Action::all()` y snapshots | Actualizar counts y regenerar snapshots revisados (AGENTS.md §5) |