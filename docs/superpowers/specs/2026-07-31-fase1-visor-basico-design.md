# Fase 1 — Visor Básico: Design Spec

> Fecha: 2026-07-31
> Proyecto: Sh_Images (visor de imágenes nativo en Rust, `egui` + `eframe`)
> Estado: Aprobado por el usuario (2026-07-31)

---

## 1. Contexto

La Fase 0 dejó la app compilando con una ventana `eframe` vacía, sistema de errores
centralizado (`ShImagesError`), configuración TOML, `core::image_loader::load_image`
(síncrono, probado), stubs documentados y benchmark base. La Fase 1 hace que la app
**muestre imágenes reales** con zoom, pan, fit-to-window y navegación por carpeta.

## 2. Alcance (in/out of scope)

### In scope
- Abrir imagen desde diálogo de archivos nativo (`rfd`), menú superior + atajo `Ctrl+O`.
- Renderizar la imagen en el canvas de `egui` con `TextureHandle`.
- Zoom con la rueda del ratón, **centrado en el cursor**.
- Pan/drag con click izquierdo + arrastrar, **pan libre** (sin clamping).
- Fit-to-window automático al abrir/cambiar imagen; atajo `F` para re-fit.
- Navegación circular (← →) entre todas las imágenes de la carpeta, filtradas por
  formatos soportados y ordenadas alfabéticamente.
- Carga asíncrona mínima (thread worker + canal `std::mpsc`) — la UI nunca se bloquea.
- Toasts simples para errores (overlay auto-desvanecido, 3s).
- Tests unitarios para: math de zoom/pan/fit (`core/view.rs`), navegación
  (`core/navigation.rs`), y fixture JPEG extra para multi-formato.

### Out of scope (fases futuras)
- LRU cache de imágenes decodificadas (Fase 2).
- Pre-carga de imagen siguiente/anterior (Fase 2).
- Miniaturas y panel lateral (Fase 2-4).
- Toolbar con iconos, rotación, pantalla completa (Fase 3).
- EXIF, slideshow (Fase 4).
- GIF animado (Fase 4).
- Carga asíncrona con pooling/pre-carga avanzado (Fase 2 refinamiento).

## 3. Decisiones de diseño (acordadas con el usuario)

1. **Carga asíncrona mínima en Fase 1**: un thread + canal `std::sync::mpsc` para no
   bloquear el UI thread (AGENTS.md §7.1). Fase 2 lo refina con pre-carga y cache.
2. **Zoom centrado en el cursor**: el punto de la imagen bajo el cursor permanece fijo
   al hacer zoom (estándar de visores).
3. **Navegación**: toda la carpeta de la imagen abierta, filtrada por extensiones de
   imagen soportadas, ordenada alfabéticamente, circular.
4. **Auto-fit + límites**: al abrir/cambiar imagen → fit. `min_zoom` = fit completo,
   `max_zoom` = 8.0 constante.
5. **Pan libre**: sin clamping; en fit la imagen queda centrada.
6. **Apertura**: menú superior "Archivo → Abrir…" + atajo `Ctrl+O` (no toolbar visible).
7. **Errores**: toast simple (overlay egui, sin crate extra) + `tracing::warn!`.
8. **Renderizado**: painter egui en `ui/viewer.rs`; toda la matemática en
   `core/view.rs` (pura, testeable, sin dependencias de egui).

## 4. Arquitectura y módulos

### Estructura resultante

```
src/core/
├── mod.rs          # añade `pub mod view;`
├── view.rs         # NUEVO — math puro de zoom/pan/fit
├── navigation.rs   # REAL — lista ordenada, next/prev circular, filtro por formato
└── image_loader.rs # se mantiene (síncrono, usado por el worker)
src/ui/
├── mod.rs          # añade `pub mod toast;`
├── viewer.rs       # REAL — painter que pinta la textura con ViewTransform
└── toast.rs        # NUEVO — overlay de notificaciones
src/app.rs          # Glue — estado, eventos, canal de carga, toasts
```

### `core/view.rs` — ViewTransform

Tipo de estado de transformación, puro (sin egui, `f32` + vectores 2D propios).

```rust
/// Transformación de vista sobre la imagen: escala y desplazamiento.
pub struct ViewTransform {
    pub zoom: f32,          // escala (1.0 = tamaño original de la imagen)
    pub pan: Vec2,          // desplazamiento en píxeles de la imagen
    pub image_size: Vec2,   // tamaño en px de la imagen actual
    pub viewport: Vec2,     // tamaño del canvas en px
}
```

> Nota: `Vec2` propio del módulo (struct `{ x: f32, y: f32 }`) para que `core/` no
> dependa de `egui`. Si se decide usar `egui::Vec2`, el módulo deja de ser puro y
> viola AGENTS.md §3.2. Se prefiere un tipo local `Point2/Vector2` mínimo.

Funciones (todas puras, sin I/O):

- `ViewTransform::new(image_size, viewport)` → fit inicial.
- `fit_zoom(image_size, viewport) -> f32` → mayor zoom que cabe sin crop.
  - Si `image_size` tiene algún eje 0 → devuelve 1.0 (protección div-by-zero).
  - Fórmula: `min(viewport.w / image.w, viewport.h / image.h)`.
- `apply_zoom_at(&mut self, anchor: Point2, factor: f32)` → cambia el zoom
  manteniendo fijo el punto de la imagen bajo `anchor`.
  - El zoom resultante se **clampa a `[fit_zoom, fit_zoom * MAX_ZOOM]`**: el mínimo
    zoom posible es el fit completo (decisión de usuario #4); el máximo es 8x el
    tamaño de fit (no un múltiplo de tamaño nativo, para que imágenes pequeñas
    tengan rango de zoom útil).
  - El pan se ajusta: `pan' = new_origin - center + image_size * new_zoom / 2`,
    donde `new_origin = anchor - image_point * new_zoom` y
    `image_point = (anchor - origin) / zoom`.
- `pan_by(&mut self, delta: Vector2)` → pan libre, sin clamp.
- `fit(&mut self)` → re-setea pan a centrado y zoom a `fit_zoom`.
- `image_origin_screen(&self) -> Point2` → esquina superior izquierda de la imagen
  en coordenadas de pantalla = `center(viewport) - image_size * zoom / 2 + pan`.
- Constantes: `MAX_ZOOM = 8.0`. No hay `MIN_ZOOM` constante: el piso es siempre el
  fit dinámico (ver `apply_zoom_at`).

### `core/navigation.rs` — Navegación

Reemplaza el stub. Estructura:

```rust
pub struct Navigation {
    pub images: Vec<PathBuf>,   // rutas absolutas, ordenadas alfabéticamente
    pub current: usize,         // índice de la imagen actual
}
```

- `Navigation::from_folder(path, supported_exts) -> Result<Navigation>`:
  - Lee el directorio (`fs::read_dir`), filtra solo archivos (no dirs) con extensión
    en `supported_exts` (case-insensitive), ordena por nombre completo (`sort_by`),
    y busca el índice del archivo `path` dentro de la lista (si no está, `current = 0`).
  - Error `ShImagesError::Io` si `read_dir` falla.
- `next(&mut self)` / `prev(&mut self)`:
  - `next`: `current = (current + 1) % len`.
  - `prev`: `current = (current + len - 1) % len`.
  - Si `images.is_empty()` → no-op (no panic).
- `current_path(&self) -> Option<&PathBuf>`.
- Lista de extensiones soportadas: `SUPPORTED_EXTENSIONS: &[&str] = ["png", "jpg",
  "jpeg", "gif", "bmp", "webp", "tiff", "tif", "avif"]`.

### `ui/viewer.rs` — Painter

```rust
pub fn show(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    transform: &mut core::view::ViewTransform,
) -> ViewResponse
```

- Alloca el rect completo del canvas (`ui.allocate_response(available_size, Sense::click_and_drag())`).
- Pinta la textura: `ui.painter().image(texture.id(), rect_image, UVRect::MAX, Color32::WHITE)`
  donde `rect_image` se deriva de `transform.image_origin_screen()` + `image_size * zoom`.
- Captura interacción:
  - **Rueda**: `ui.input(|i| i.raw_scroll_delta.y)` sobre el hover → llama a
    `transform.apply_zoom_at(anchor, factor)` con `factor = exp(-scroll_y * 0.001)`
    o similar. Marca `zoomed` en la respuesta.
  - **Drag**: `response.drag_delta()` → `transform.pan_by(delta)`. Marca `panned`.
  - `response.request_focus()` en hover para capturar teclado.
- Devuelve `ViewResponse { zoomed: bool, panned: bool }` (para que app.rs sepa si el
  usuario interactuó y decida auto-fit vs. mantener estado).

> `ui.viewer.rs` NO decide navegación ni carga; solo transformación + reporte.

### `ui/toast.rs` — Toasts

```rust
pub struct Toasts {
    items: Vec<Toast>,
}
struct Toast { message: String, expires_at: f64 }  // f64 = seconds since frame start
```

- `push(&mut self, message: String)` → agrega con `expires_at = now + 3.0`.
- `update(&mut self, now: f64)` → elimina expirados.
- `show(&self, ui: &mut egui::Ui)` → dibuja overlay en esquina inferior derecha
  (área centrada, `egui::Window` sin título, `order(Order::Foreground)`, `frame`
  con fondo), una fila por toast, sin interactividad.
- Los tiempos se toman de `ui.input(|i| i.time)` (reloj de egui).

### `src/app.rs` — Glue

Estado añadido a `ShImagesApp`:

```rust
struct LoadedImage { image: DynamicImage, path: PathBuf }
pub struct ShImagesApp {
    settings: Settings,
    navigation: Option<Navigation>,        // None si no hay carpeta abierta
    transform: ViewTransform,              // estado de zoom/pan (con image_size 0 si vacío)
    texture: Option<egui::TextureHandle>,
    rx: Option<std::sync::mpsc::Receiver<LoadEvent>>,
    toasts: Toasts,
    user_interacted: bool,                 // si el usuario tocó zoom/pan manualmente
}
```

Flujo:

- **Abrir (Ctrl+O / menú)**: `rfd::FileDialog::new().add_filter("Imágenes", &["png","jpg","jpeg",...]).pick_file()`.
  - `pick_file()` es bloqueante pero se invoca en el UI thread y es un diálogo nativo
    modal; es el patrón estándar de `rfd` + egui. (El worker async es para la
    decodificación de la imagen, no para el diálogo.)
  - Con la ruta: `Navigation::from_folder(parent, SUPPORTED_EXTENSIONS)` y spawn worker.
- **Spawn worker** (`fn start_load(&mut self, path)`):
  - Crea canal `mpsc::channel::<LoadEvent>()`, spawn `std::thread::spawn(move || load_image(&path))`.
  - El worker carga y envía `LoadEvent { path, result }` (para saber a qué ruta
    pertenece y no aplicar resultados obsoletos).
  - Guarda `rx` en `self`.
- **Cada frame**: `if let Ok(event) = rx.try_recv() { ... }`:
  - Si es la imagen actual esperada → `self.texture = Some(make_texture(&event.image))`,
    `self.transform = ViewTransform::new(...)`, `self.user_interacted = false`,
    `self.navigation.current` ya fue actualizado al spawnear.
  - Si `Err` → `self.toasts.push(msg)` + `tracing::warn!`.
  - Si llega un evento de una navegación antigua (path != current) → se ignora.
- **Interacción en el frame**: `viewer::show(...)` reporta `zoomed/panned` →
  `user_interacted = true`. Si el usuario no ha interactuado y la ventana
  redimensiona → auto re-fit (`transform.fit()`).
- **Navegación** (← → o menú): `navigation.next()/prev()` → `start_load(current_path)`.
- **F**: `transform.fit()`.
- **make_texture**: `ColorImage::from_rgba_unmultiplied` (si la imagen tiene canal
  alpha; si es RGB sin alpha, `from_rgb`) → `ctx.load_texture("img", color_image, TextureOptions::LINEAR)`.

## 5. Dependencias nuevas

- **`rfd`** (en `Cargo.toml`, dependencia regular): diálogo de archivos nativo.
  - Justificación: estándar en el ecosistema egui, nativo por plataforma, sin runtime
    pesado. Mantenido, MIT/Apache-2.0.
  - Versión: `rfd = "0.15"` (verificar la última estable al implementar).

No se agregan otras dependencias. (Los toasts usan egui puro; el canal usa std.)

## 6. Formato del diálogo

- `rfd::FileDialog::new().add_filter("Imágenes", SUPPORTED_EXTENSIONS)` — solo
  imágenes. Fallback: sin filtro para "Todos los archivos" (opcional).

## 7. Testing

### `core/view.rs`
- `fit_zoom_scales_to_fit_wide_image` (imagen ancha → zoom limita por altura).
- `fit_zoom_scales_to_fit_tall_image` (imagen alta → zoom limita por anchura).
- `fit_zoom_returns_1_on_zero_dimension`.
- `apply_zoom_at_keeps_anchor_point_fixed`.
- `apply_zoom_at_clamps_to_max_zoom` (max = 8x fit) y al fit como mínimo.
- `pan_by_moves_image_and_reports`.
- `fit_resets_to_centered_initial`.

### `core/navigation.rs`
- `from_folder_filters_by_extension` (carpeta con PNG + .txt + .JPG).
- `from_folder_sorts_alphabetically`.
- `from_folder_sets_current_to_matching_path` / `falls_back_to_zero_if_not_found`.
- `next_wraps_circularly`.
- `prev_wraps_circularly`.
- `next_on_empty_is_noop` (no panic).
- `from_folder_returns_io_error_on_missing_dir`.

### `ui/toast.rs`
- `push_adds_toast_with_expiry`.
- `update_removes_expired_toasts`.
- (Si es testeable headless: `show` no panics con lista vacía.)

### `core/image_loader.rs`
- Añadir fixture `tests/fixtures/sample.jpg` (JPEG 16×16) y test
  `decoding_valid_jpeg_returns_image`.

### Fixtures
- `tests/fixtures/sample.jpg`: JPEG 16×16 válido (agregado en Fase 1; archivo
  pequeño, < 100KB).

## 8. Criterios de aceptación

1. `cargo run` abre la ventana; Ctrl+O abre el diálogo nativo; seleccionar una
   imagen la muestra con auto-fit.
2. Rueda → zoom centrado en cursor, limitado a `[fit, 8.0]`.
3. Drag → pan libre.
4. ← → navega circularmente por la carpeta (solo imágenes), auto-fit al cambiar.
5. `F` re-hace fit.
6. Imagen corrupta/no soportada → toast + `tracing::warn!`, sin crash.
7. Redimensionar ventana re-fitea mientras no haya interacción manual.
8. Suite: `cargo check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo fmt --check`, `cargo test`, `cargo test --release` pasan.
9. Cobertura de `core/view.rs` y `core/navigation.rs` ≥ 80% (AGENTS.md §4.1).
10. Sin `unsafe`, sin `unwrap`/`expect` en producción, docstrings en todo lo público.

## 9. Fuera de alcance documentado

- No se implementa cache, pre-carga ni pooling de threads en esta fase (Fase 2).
- El diálogo `pick_file` es bloqueante por diseño de `rfd` (modal nativo); la
  decodificación es async. No se usa `AsyncFileDialog` para no añadir polling extra
  de futures en Fase 1.
