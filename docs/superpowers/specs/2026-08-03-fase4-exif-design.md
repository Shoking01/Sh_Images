# Fase 4 — EXIF: Lectura de metadatos y panel de información — Design

> Fecha: 2026-08-03 · Estado: Aprobado por usuario · Alcance: sub-features 1 y 2
> de la Fase 4 de Plan.md (GIF animado y slideshow quedan como sub-fases posteriores).

## 1. Contexto

Sh_Images tiene completas las Fases 0–3 (fundamentos, visor básico, cache y
rendimiento, UI/UX polish). El visor abre imágenes, navega con ←→, hace zoom/fit,
rota, tiene sidebar de miniaturas, pre-carga N±1, atajos configurables, tema y
snapshot testing con `insta`.

La Fase 4 (Plan.md §4) es Metadatos Avanzados. Esta spec cubre las dos primeras
sub-features:

1. **Lectura de metadatos EXIF** — `src/core/exif.rs` es hoy un stub vacío de 1
   línea.
2. **Panel lateral con info EXIF** — el sidebar izquierdo hoy solo muestra
   miniaturas; la info EXIF irá en un panel derecho dedicado.

GIF animado y slideshow automático NO son parte de esta spec (sub-fases futuras).

## 2. Decisiones de diseño

### 2.1 Dependencia: `kamadak-exif`

**Contexto (AGENTS.md §7.2):** necesitamos parsear metadatos EXIF de JPEG/TIFF.
Plan.md §6 ya recomienda `kamadak-exif` como mitigación para la complejidad EXIF.

**Decisión:** `kamadak-exif = "0.6"` en `[dependencies]`, con comentario de
justificación en Cargo.toml (licencia MIT/Apache-2.0, mantenido, estándar del
ecosistema, explicit en Plan.md).

**Alternativas consideradas:** `kamadak` (crate hermano, menos mantenido), parser
manual del bloque APP1/TIFF (frágil, reimplantar el estándar → descartado).

### 2.2 Formato: JPEG + TIFF; el resto muestra "Sin metadatos"

**Decisión:** se parsea EXIF de JPEG (bloque APP1) y TIFF (el mismo crate cubre ambos).
Es donde vive el 99% del EXIF de cámaras. Imágenes sin EXIF (PNG, BMP, WebP) o
JPEG sin APP1 devuelven `Ok(None)` y el panel muestra "Sin metadatos EXIF". No se
añade HEIF/RAW en esta fase (superficie de errores y dependencias extra).

### 2.3 Modelo curado `ExifImage`

**Decisión:** un struct con campos `Option<T>` de 8 etiquetas (no un dump crudo),
fácil de testear y formatear.

```rust
pub struct ExifImage {
    pub make: Option<String>,          // IFD0 Make
    pub model: Option<String>,        // IFD0 Model
    pub date_time: Option<String>,    // EXIF DateTimeOriginal (formateado)
    pub iso: Option<u32>,             // EXIF ISO
    pub f_number: Option<Rational>,   // EXIF FNumber
    pub shutter_speed: Option<Rational>, // EXIF exposure_time (inverso)
    pub focal_length: Option<Rational>,  // EXIF FocalLength
    pub orientation: Option<u16>,    // IFD0 Orientation (1..=8)
}
```

`orientation` se expone como `u16` crudo; la presentación (icono "Vertical"/
"Horizontal") es responsabilidad del UI. Esto mantiene `core/` libre de presentación.

### 2.4 `Rational` y formateo legible

**Decisión:** tipo auxiliar `Rational` (numerador/denominador) con `to_string()`
produciendo formato humano: `"f/2.8"`, `"1/125 s"`, `"50 mm"`. El *source* es la
exposición/distancia focal; el formato legible se usa para FNumber, ExposureTime
y FocalLength. El formateo es puro y testeable con `insta`.

### 2.5 Lectura asíncrona con cache

**Decisión (AGENTS.md §7.1 "no bloquear el UI thread"):** un worker dedicado que
lee el EXIF (I/O) en segundo plano y notifica a la UI, con un pequeño cache por
path para no releer al volver a una imagen (evicción al cambiar de carpeta). Este
pipeline recibe un canal de datos vía el mismo patrón que `poll_thumbnails`.

### 2.6 Gestión de estado vs. cache

Dos planos:

- **Cache persistente** (`ExifCache`, `Arc<Mutex<HashMap<PathBuf, ExifRead>>>`):
  clave por path, valor `ExifRead` (distinguir `Found(img)` de `None` y de
  `Err(_)` sin re-parsear). Ninguna evicción LRU compleja: simplemente se limpia
  al abrir una carpeta nueva (mismo `clear` que el `ThumbnailCache`).
- **Estado del panel** (`InfoPanelState`): solo `show: bool` (visible/oculto).

### 2.7 Campo `ShImagesError::Exif(String)`

**Decisión:** añadir variante `Exif(String)` a `ShImagesError` para errores de
parse/unsupported de `kamadak-exif` sin acoplar el crate a la UI (el *from*
automático de `io::Error` sigue cubriendo los fallos de lectura del filesystem).

## 3. Componentes

### 3.1 `src/core/exif.rs` (REESCRIBIR stub)

- `struct Rational { num: u64, den: u64 }` con `new()`, `to_decimal()` y
  `to_string()` legible.
- `struct ExifImage` (campos del §2.3).
- `enum ExifRead { Found(ExifImage), None, Err(ShImagesError) }`.
- `pub fn read_exif(path: &Path) -> Result<Option<ExifImage>, ShImagesError>`:
  abre el archivo, lee las tablas EXIF con `kamadak-exif`, mapea a `ExifImage`.
  `Ok(None)` si no hay bloque APP1/IFD0. Mapea errores de kamadak a
  `ShImagesError::Exif`.

### 3.2 `src/ui/info_panel.rs` (CREAR)

```rust
pub struct InfoPanelState { pub show: bool }
```

- `pub fn show(ui, cache, current_path) -> bool` — pinta `egui::Panel::right`
   con scroll vertical y devuelve `true` si el panel capturó un click (para no
   dejar que la imagen lo engulla).
- Pinta título "Información" + los campos curados no-ausentes con
   `ui.label` / `ui.heading` en Columnas/CollapsingHeader.
- Según `ExifRead`:
   `Found(img)` → filas campo:valor.
   `None` → "Sin metadatos EXIF".
   `Err(e)` → "No se pudo leer los metadatos" (y toast desde app).
- Formateo puro y testeable: `fn field_rows(img: &ExifImage) -> Vec<(String,String)>`
   y `fn format_rational(r: &Rational) -> String` (usado por `field_rows`).

### 3.3 `src/core/actions.rs` (MODIFICAR)

- Añadir variante `Action::ToggleInfo` (`label()` = `"Info"`, `default_shortcut()`
  = `Some(KeyBinding::new(KeyCode::KeyI, Modifiers::NONE))`).
- `Action::all()` enumera la variante automáticamente (Fase 3 garantiza que el
  editor de atajos y la toolbar ya la cubren).

### 3.4 `src/config/settings.rs` (MODIFICAR)

- Sin cambios de schema. `#[serde(default)]` sigue cubriendo la migración. `ToggleInfo`
  va en el `ShortcutMap` con default, no en `Settings` directamente.

### 3.5 `src/app.rs` (MODIFICAR)

- Nuevos campos: `exif_cache: Arc<Mutex<HashMap<PathBuf, ExifRead>>>`,
  `exif_tx: mpsc::Sender<PathBuf>`, `exif_rx: Option<mpsc::Receiver<PathBuf>>`,
  `info_panel: InfoPanelState`.
- `need_exif(&self, path)` en `apply_decoded`: si `exif_cache.get(path)` es
  `None` (miss) y no está en-flight, encolar `path` al worker.
- `poll_exif(&mut self)`: drena el canal; por cada path re-lee `exif_cache` y
  hace `request_repaint()` para que el panel se repinte.
- Worker pool (1 thread): `while let Some(path)` del canal → `read_exif` →
  `insert(cache, path, resultado)`. (Un solo worker basta, el EXIF es barato.)
- `dispatch(Action::ToggleInfo)` → `info_panel.show = !info_panel.show`.
- `ui()`: llama `info_panel.show(...)` dentro de un `Panel::right` si `show`.
- En `open_path`, limpia `exif_cache` (invalidate) igual que thumbnail cache.

### 3.6 `src/ui/mod.rs` (MODIFICAR) y `src/core/mod.rs` (sin cambios)

- Registrar el módulo `info_panel` en `ui/mod.rs`. `core/mod.rs` ya registra
  `exif`; no requiere cambios.

## 4. Data flow

```
open/navigate → apply_decoded(path)
   └─ need_exif(path)  → cache hit? no → enqueue path → worker → read_exif → insert cache → notify
frame n+1:  poll_exif → reap → request_repaint
frame n+2:  info_panel.show(&cache, current_path) → muestra campos
```

- Cache hit sobre un path ya visto: no se re-encola, panel muestra al instante.
- `ExifRead::None` cacheado: el panel muestra "Sin metadados" sin re-parse.

## 5. Error handling

- `read_exif` devuelve `Result<Option<ExifImage>>`. En el worker, cualquier error
  se cachea como `ExifRead::Err`.
- En la UI, `ExifRead::Err(e)` → toast "No se pudo leer los metadatos de la
  imagen" (sin exposurar la cadena interna) + el panel muestra un placeholder.
- `ShImagesError::Exif(String)` (nuevo) — mapea errores de kamadak-exif.
- Nunca `.unwrap()`/`.expect()` en producción (AGENTS.md §2.1): acceso a cache con
  `unwrap_or_else(|p| p.into_inner())` como en los otros caches.

## 6. Testing

### 6.1 Unitarios resolver `core/exif.rs`

- `read_exif` de un JPEG con tags → `ExifImage` con campos poblados (Make/Model,
  DateTime, ISO, FNumber, ExposureTime, FocalLength, Orientation).
- JPEG sin APP1 (PNG generado) → `Ok(None)`.
- TIFF con EXIF → `Ok(Some(...))`.
- Archivo corrupto → `Err(ShImagesError::Exif(_))`.
- `Rational::to_string` casos (f/2.8, 1/125 s, 50 mm) y límites.

> Generación del JPEG con EXIF real: usar `kamadak-exif::Writer` para producir un
> JPEG sintético determinado con tags en el setup del test; evita commitear un
> binario (AGENTS.md §8.2).

### 6.2 Snapshots `insta`

- `exif_rows` — `format_rows` de un `ExifImage` (campos poblados y con ausencias).
- `rational_to_string` — tabla de casos de `format_rational`.

### 6.3 Integración

- Ampliar `tests/integration.rs` con flujo `flujo_exif`: abrir imagen con EXIF →
  leer `read_exif` → verificar un campo clave (no Crash). Además: abrir PNG sin
  EXIF → `Ok(None)`.

### 6.4 Cobertura

- `core/exif` ≥ 90%.
- `ui/info_panel` formateo puro (`format_rows` / `format_rational`) ≥ 70%; el
  código GPU de `Panel::right` queda como QA manual visual.

## 7. Files afectados

```
src/core/exif.rs           MODIFICAR — reescribir stub (model + read_exif)
src/core/actions.rs        MODIFICAR — Action::ToggleInfo + shortcut
src/ui/info_panel.rs      CREAR — panel derecho
src/ui/mod.rs             MODIFICAR — registrar info_panel
src/app.rs                MODIFICAR — exif worker + cache + dispatch + panel
src/utils/errors.rs       MODIFICAR — variante Exif(String)
Cargo.toml                MODIFICAR — kamadak-exif
tests/integration.rs       MODIFICAR — flujo EXIF
tests/common/mod.rs        MODIFICAR — fixture JPEG con EXIF sintético
docs/ARCHITECTURE.md      MODIFICAR — ADR de EXIF
```

## 8. Riesgos

| Riesgo | Mitigación |
|--------|-----------|
| `kamadak-exif` no detecta APP1 en algún JPEG | Tests con JPEG sintético real; QA manual; tratar como `Ok(None)` sin crash |
| Orientación: unas imágenes con `orientation=6` deberían girarse | Esta fase solo *expone* el campo; la rotación automática por orientación es decisión futura, documentada en ADR |
| Parser falla en TIFF exótico | Se cachea `Err` y se muestra toast; nunca crash |
| El worker simple del canal se llena para carpetas enormes | Solo el worker entrega por path; drenable; el cache se limpia al cambiar carpeta |