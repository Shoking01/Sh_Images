# Fase 2 — Subproyecto 3: Miniaturas y Sidebar: Design Spec

> Fecha: 2026-08-01
> Proyecto: Sh_Images (visor de imágenes nativo en Rust, `egui` + `eframe`)
> Estado: Aprobado por el usuario (2026-08-01)

---

## 1. Contexto

Los subproyectos 1 y 2 de Fase 2 implementaron el cache LRU (`core/image_cache.rs`)
y lo conectaron al flujo de carga de `app.rs` con pre-carga N±1 (canal único +
workers + `poll_loader`). El visor ya navega con `←`/`→` de forma inmediata para
imágenes cacheadas o pre-cargadas.

Este subproyecto añade las **miniaturas de la carpeta** y la **barra lateral
(sidebar)** que las muestra: una grid de miniaturas (~96px) con highlight de la
imagen actual y click para saltar a cualquier imagen de la carpeta.

Los stubs existentes son `src/core/thumbnail_gen.rs` (generación de miniaturas) y
`src/ui/sidebar.rs` (panel lateral) — ambos de una sola línea, a implementar aquí.

## 2. Alcance (in/out of scope)

### In scope
- Módulo `core/thumbnail_gen.rs`: `thumbnail_size()` y `generate_thumbnail()`
  (funciones puras de downscale, sin I/O ni threads).
- Módulo `core/thumbnail_cache.rs`: `ThumbnailCache` en memoria (HashMap thread-safe,
  sin evicción LRU).
- Pipeline de miniaturas en `app.rs`: canal `mpsc` independiente del de S2 + pool
  acotado de workers.
- Módulo `ui/sidebar.rs`: grid de miniaturas, highlight del actual, click-navega,
  placeholder para miniaturas pendientes, toggle para ocultar el sidebar.
- Tests unitarios para `thumbnail_gen`, `thumbnail_cache` y helpers de `ui/sidebar`
  extraídos a `core`.
- QA: `cargo check`, `clippy`, `fmt`, `test`, `test --release`.

### Out of scope (YAGNI en este subproyecto)
- Persistencia de miniaturas en disco.
- Virtualización (generar solo las visibles) — no necesario para decenas a cientos.
- Metadatos/EXIF en el sidebar.
- Sidebar con tamaño de miniatura ajustable.
- Cualquier cambio al pipeline full-res de S2 (cache LRU, pre-carga N±1).

## 3. Decisiones de diseño (acordadas con el usuario)

1. **Pipeline de miniaturas separado del full-res**: canal `mpsc` propio +
   `ThumbnailCache` independiente. No reutilizar el `ImageCache` LRU de S2 para
   miniaturas: decodificar *todas* las imágenes de la carpeta a resolución completa
   para luego downscalear llenaría el LRU (512 MiB) y evictaría la imagen actual.
   El pipeline full-res de S2 queda intacto.
2. **Miniaturas solo en memoria**: `ThumbnailCache` es un
   `Arc<Mutex<HashMap<PathBuf, DynamicImage>>>`. Sin evicción LRU: a 96px cada
   miniatura es ~37 KB (RGBA 96×96×4); cientos de imágenes = decenas de MB,
   despreciable frente al límite de 512 MiB del visor. Al abrir una carpeta se
   hace `clear()` y se regeneran.
3. **Pool acotado de workers** (2–4 threads): nunca un thread por imagen. Spawnea
   N threads al abrir la carpeta que consumen de un canal de paths. Evita el
   desperdicio de decenas/hundreds de threads decodificando simultáneamente.
4. **Best-effort silencioso**: si falla la miniatura de una imagen (archivo
   corrupto, no soportado), se descarta silenciosamente (`tracing::debug!`) y la
   celda muestra placeholder gris. No se tostifica — los toasts son para el visor
   (imagen actual), no para miniaturas background.
5. **Click-navega**: al hacer click en una miniatura se pone
   `navigation.current = i` y se llama `start_load(path)`, reutilizando el flujo
   de S2 (cache hit → instantáneo; miss → worker). La UI no duplica lógica de
   carga.
6. **`ui/` sin lógica**: `sidebar.rs` solo presenta. La lógica de grid (cálculo de
   columnas, selección de celdas) se extrae como funciones puras en `core/` (o en
   un helper testeable) para cumplir el estándar de cobertura de AGENTS.md §4.1
   (UI helpers ≥ 70%).

## 4. Arquitectura y módulos

### Estructura resultante

```
src/
├── app.rs                     # Estado global y loop principal (modificado)
├── core/
│   ├── thumbnail_gen.rs       # IMPLEMENTAR: thumbnail_size + generate_thumbnail
│   ├── thumbnail_cache.rs     # NUEVO: ThumbnailCache (HashMap thread-safe)
│   └── sidebar_layout.rs      # NUEVO (opcional): helpers puros de grid
└── ui/
    └── sidebar.rs             # IMPLEMENTAR: grid + highlight + click + toggle
```

### `core/thumbnail_gen.rs`

```rust
/// Devuelve el tamaño de miniatura manteniendo el aspect ratio.
/// Nunca amplía: si la imagen ya cabe en `max`, devuelve las dimensiones originales.
pub fn thumbnail_size(w: u32, h: u32, max: u32) -> (u32, u32) { /* ... */ }

/// Genera una miniatura de `image` con el lado mayor = `max`.
pub fn generate_thumbnail(image: &DynamicImage, max: u32) -> DynamicImage { /* ... */ }
```

- `thumbnail_size(_, _, 0)` → `(0, 0)` (sin validación panicky; es input público).
- `generate_thumbnail(_, 0)` devuelve la imagen original sin modificar: `image.thumbnail`
  con dimensión 0 no está definida en el crate `image` y podría panic.
- `thumbnail_size` se calcula con aritmética de `u64` para evitar overflow en
  dimensiones grandes.

### `core/thumbnail_cache.rs`

```rust
/// Cache en memoria de miniaturas (sin evicción). Thread-safe.
pub struct ThumbnailCache { inner: Arc<Mutex<HashMap<PathBuf, DynamicImage>>> }

impl ThumbnailCache {
    pub fn new() -> Self;
    pub fn insert(&self, path: PathBuf, image: DynamicImage);
    pub fn get(&self, path: &Path) -> Option<ThumbnailRef<'_>>;  // Deref<DynamicImage>
    pub fn contains(&self, path: &Path) -> bool;
    pub fn clear(&self);
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

- `get` devuelve una ref que mantiene el lock (mismo patrón que `CacheEntryRef` de
  `image_cache.rs`) para no clonar pixels en el UI thread.
- `clear()` se llama al abrir una carpeta distinta.

### Pipeline en `app.rs`

- Nuevos campos:
  ```rust
  thumb_cache: Arc<ThumbnailCache>,
  thumb_tx: mpsc::Sender<PathBuf>,          // canal de paths a procesar
  thumb_rx: mpsc::Receiver<PathBuf>,        // lado worker: consume paths
  thumb_events_rx: Option<mpsc::Receiver<()>>,  // UI: notificación de miniatura lista
  thumb_events_tx: mpsc::Sender<()>,
  ```
  (o variante: el worker envía `PathBuf` procesado por `thumb_events` para logging;
  la UI solo necesita un "repaint").

- Al abrir carpeta (`open_path` → `Navigation::from_folder` OK):
  - `thumb_cache.clear()`.
  - Encolar todos los `nav.images` en `thumb_tx`.
- Workers del pool (spawned en `new()` o lazy en primera carpeta):
  ```rust
  for _ in 0..POOL_SIZE {  // POOL_SIZE = 3 (default)
      let rx = thumb_rx.clone();
      let cache = thumb_cache.clone();
      let events_tx = thumb_events_tx.clone();
      std::thread::spawn(move || {
          while let Ok(path) = rx.recv() {
              let result = load_image(&path)
                  .map(|img| cache.insert(path.clone(), generate_thumbnail(&img, THUMB_MAX)));
              // best-effort: error → solo tracing::debug!, sin toast
              if let Err(e) = &result { tracing::debug!(error = %e, path = %path.display(), "thumbnail failed"); }
              if events_tx.send(()).is_err() { tracing::debug!("thumbnail event dropped (receiver gone)"); }
          }
      });
  }
  ```
  Nota: `mpsc::Receiver` es `Sync` si el tipo lo es; `PathBuf` y `DynamicImage` lo
  son → se puede clonar `rx` por worker con `Arc<Receiver>` o `Mutex<Receiver>`.
  (Detalle de implementación; el plan elegirá la mecánica exacta.)
- `poll_thumbnails()` en el frame: drena `thumb_events_rx`; no hace nada más que
  `ctx.request_repaint()` (la UI lee `thumb_cache` directamente).
- `POOL_SIZE = 3`, `THUMB_MAX = 96` como constantes en `core/` o `app.rs`.

### `ui/sidebar.rs`

- `egui::SidePanel::left("sidebar").show(ctx, ...)` renderizado SOLO si
  `show_sidebar` es `true`.
- Grid: `ui.horizontal_wrapped` (o `egui::Grid` según lo que decida el plan) con
  celdas de `THUMB_MAX + padding`.
- Cada celda:
  - Si `thumb_cache.get(path)` → imagen (miniaturas del mismo aspect ratio,
    centradas en la celda).
  - Si no → placeholder gris con texto "…".
  - Highlight (marco) si `i == navigation.current`.
  - `response.clicked()` → `app.navigate_to(i)` (setea `current` + `start_load`).
- Toggle: botón "Sidebar" en la barra de menú o atajo `H`.
- Extraer a `core/sidebar_layout.rs` (o módulo helper) la lógica pura de grid:
  `columns_for_width(width, cell, spacing) -> usize` y `cell_index/position` — para
  poder testear el layout sin egui.

## 5. Flujo de datos (resumen)

```
Abrir carpeta (open_path)
  └─ Navigation::from_folder → nav (M imágenes)
  └─ thumb_cache.clear()
  └─ encolar todos los nav.images en thumb_tx
       └─ pool workers: recv path → load_image → generate_thumbnail(96)
                          → thumb_cache.insert → events_tx.send(()) → repaint
       └─ poll_thumbnails: drena events → repaint
  └─ sidebar muestra grid: celda_i = thumb_cache.get(images[i])
       └─ pendientes → placeholder gris
       └─ actual (i == current) → marco highlight
Usuario click en miniatura i
  └─ navigation.current = i
  └─ start_load(images[i])   // reutiliza S2: cache hit → instantáneo
```

## 6. Testing

### 6.1 `core/thumbnail_gen.rs`
- `thumbnail_size` mantiene aspect ratio en imagen ancha, alta y cuadrada.
- `thumbnail_size` no amplía imágenes más pequeñas que `max`.
- `thumbnail_size` con `max = 0` devuelve `(0, 0)`.
- `thumbnail_size` con `w` o `h` = 0 devuelve `(0, 0)` (sin panic/overflow).
- `generate_thumbnail` produce una imagen con las dimensiones esperadas.

### 6.2 `core/thumbnail_cache.rs`
- `insert` + `get` roundtrip (dimensiones correctas).
- `get` sobre path ausente devuelve `None`.
- `contains` true/false.
- `clear` vacía el cache; `len`/`is_empty` reflejan el estado.
- Sobreescritura de la misma key reemplaza la entrada (sin duplicados).

### 6.3 `core/sidebar_layout.rs` (helpers puros)
- `columns_for_width` calcula el número de columnas correcto según el ancho.
- Devuelve ≥ 1 siempre (nunca 0).

### 6.4 Suite previa
- Los 75 tests existentes (Fase 0/1 + S1 + S2) siguen en verde; la API de
  `ImageCache`, `Navigation`, `preload` y `app.rs` existente no cambia salvo por
  la adición de campos/canales de miniaturas.

## 7. Criterios de éxito

- Al abrir una carpeta, el sidebar muestra una grid de miniaturas que se rellena
  progresivamente; la imagen actual está resaltada.
- Click en una miniatura salta a esa imagen en el visor (cache hit si ya estaba
  decodificada; worker si no).
- El toggle oculta/muestra el sidebar y el atajo `H` funciona.
- `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo test`
  y `cargo test --release` pasan sin warnings.
- Sin `.unwrap()`/`.expect()` en producción.
- `core/` sin dependencias de `egui`.
- No hay regresión en el pipeline full-res de S2 (navegación `←`/`→` sigue
  instantánea para imágenes cacheadas/pre-cargadas).
