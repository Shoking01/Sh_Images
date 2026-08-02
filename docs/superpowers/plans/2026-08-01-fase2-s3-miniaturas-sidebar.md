# Fase 2 — Subproyecto 3: Miniaturas y Sidebar — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Añadir miniaturas de la carpeta y una barra lateral (sidebar) con grid de miniaturas (~96px), highlight de la imagen actual, click para saltar a cualquier imagen, placeholder para pendientes y toggle para ocultar.

**Architecture:** Pipeline de miniaturas **independiente** del full-res de S2: `ThumbnailCache` en memoria (`Arc<Mutex<HashMap<PathBuf, DynamicImage>>>`, sin evicción LRU) + canal `mpsc` de paths a un pool acotado de workers (3 threads) + canal de eventos solo para `request_repaint`. `core/thumbnail_gen.rs` expone `thumbnail_size` y `generate_thumbnail` (puras). `ui/sidebar.rs` solo presenta: construye texturas de egui desde el cache y devuelve el índice seleccionado; la lógica de carga la hace `app.rs` vía `start_load` (S2).

**Tech Stack:** `image::DynamicImage::thumbnail`, `std::sync::{Arc, Mutex, mpsc}`, `eframe::egui` (`SidePanel`, `horizontal_wrapped`, `painter().image`). Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-08-01-fase2-s3-miniaturas-sidebar-design.md`

---

## File Structure

```
src/core/thumbnail_gen.rs   # IMPLEMENTAR — THUMB_MAX, thumbnail_size, generate_thumbnail (+ tests)
src/core/thumbnail_cache.rs # CREAR — ThumbnailCache (HashMap thread-safe) (+ tests)
src/core/mod.rs             # MODIFICAR — registrar thumbnail_cache
src/ui/sidebar.rs           # IMPLEMENTAR — SidebarState::show, fit_thumbnail (+ tests)
src/app.rs                  # MODIFICAR — pipeline de miniaturas + pool + integrate sidebar
```

No se toca `Cargo.toml` (sin dependencias nuevas).

> **Decisión (deviation del spec):** el spec listaba `core/sidebar_layout.rs` como "(opcional)" para helpers de grid (`columns_for_width`). **No se crea**: la grid usa `egui::ui.horizontal_wrapped`, que reparte columnas automáticamente según el ancho — un helper `columns_for_width` sería código muerto (lección del S2 con `neighbor_paths`). La única lógica extraíble real (ajuste de aspect ratio de la miniatura en su celda) es `fit_thumbnail`, que vive en `ui/sidebar.rs` con tests unitarios propios.

---

## Task 1: `core/thumbnail_gen.rs` — `thumbnail_size` y `generate_thumbnail` (TDD)

**Files:**
- Modify: `src/core/thumbnail_gen.rs` (reescribir el stub de 1 línea)

- [ ] **Step 1: Escribir los tests que fallan**

Reemplazar TODO el contenido actual de `src/core/thumbnail_gen.rs` (solo el docstring `//! Generación de miniaturas (implementación completa en Fase 2).`) por:

```rust
//! Generación de miniaturas: downscale puro sin I/O ni threads.
//!
//! `core/` no depende de `egui` (AGENTS.md §3.2). La `DynamicImage` viene del
//! crate `image`, ya presente.

use image::DynamicImage;

/// Tamaño por defecto del lado mayor de una miniatura (px).
pub const THUMB_MAX: u32 = 96;

/// Devuelve el tamaño de miniatura manteniendo el aspect ratio.
///
/// Nunca amplía: si la imagen ya cabe en `max`, devuelve las dimensiones
/// originales. `(0, 0)` si `max`, `w` o `h` es `0`.
///
/// Se calcula en `f64` para evitar overflow en dimensiones grandes.
pub fn thumbnail_size(w: u32, h: u32, max: u32) -> (u32, u32) {
    if max == 0 || w == 0 || h == 0 {
        return (0, 0);
    }
    let (w, h) = (w as f64, h as f64);
    let max = max as f64;
    if w <= max && h <= max {
        return (w as u32, h as u32);
    }
    let scale = max / w.max(h);
    let nw = (w * scale).round() as u32;
    let nh = (h * scale).round() as u32;
    (nw.max(1), nh.max(1))
}

/// Genera una miniatura de `image` con el lado mayor = `max`.
///
/// Con `max == 0` devuelve la imagen original sin modificar: `DynamicImage::thumbnail`
/// con dimensión 0 no está definida y podría panic. Igual si la imagen ya cabe.
pub fn generate_thumbnail(image: &DynamicImage, max: u32) -> DynamicImage {
    let (w, h) = image.dimensions();
    let (nw, nh) = thumbnail_size(w, h, max);
    if nw == 0 || (nw == w && nh == h) {
        return image.clone();
    }
    image.thumbnail(nw, nh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, RgbaImage};

    fn rgba(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_wide() {
        assert_eq!(thumbnail_size(1920, 1080, THUMB_MAX), (96, 54));
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_tall() {
        assert_eq!(thumbnail_size(1080, 1920, THUMB_MAX), (54, 96));
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_square() {
        assert_eq!(thumbnail_size(100, 100, THUMB_MAX), (96, 96));
    }

    #[test]
    fn thumbnail_size_does_not_upscale() {
        assert_eq!(thumbnail_size(50, 30, THUMB_MAX), (50, 30));
    }

    #[test]
    fn thumbnail_size_zero_max_returns_zero() {
        assert_eq!(thumbnail_size(1920, 1080, 0), (0, 0));
    }

    #[test]
    fn thumbnail_size_zero_dimension_returns_zero() {
        assert_eq!(thumbnail_size(0, 1080, THUMB_MAX), (0, 0));
        assert_eq!(thumbnail_size(1920, 0, THUMB_MAX), (0, 0));
    }

    #[test]
    fn thumbnail_size_never_returns_zero_for_small_dimensions() {
        assert_eq!(thumbnail_size(1, 1, THUMB_MAX), (1, 1));
        assert_eq!(thumbnail_size(1920, 1, THUMB_MAX), (96, 1));
    }

    #[test]
    fn generate_thumbnail_downscales_to_max() {
        let img = generate_thumbnail(&rgba(1920, 1080), THUMB_MAX);
        assert_eq!(img.dimensions(), (96, 54));
    }

    #[test]
    fn generate_thumbnail_small_image_unchanged() {
        let img = generate_thumbnail(&rgba(50, 30), THUMB_MAX);
        assert_eq!(img.dimensions(), (50, 30));
    }

    #[test]
    fn generate_thumbnail_zero_max_returns_original() {
        let img = generate_thumbnail(&rgba(1920, 1080), 0);
        assert_eq!(img.dimensions(), (1920, 1080));
    }
}
```

- [ ] **Step 2: Ejecutar los tests para ver que fallan**

Run: `cargo test thumbnail_gen`
Expected: FAIL — `error[E0425]: cannot find function 'thumbnail_size'` (el módulo no tiene ninguna función aún).

- [ ] **Step 3: Verificar que el downscale produce las dimensiones esperadas**

El código del Step 1 ya incluye la implementación. Verificar el razonamiento de `thumbnail_size(1920, 1080, 96)`: `scale = 96/1920 = 0.05`; `nw = 1920*0.05 = 96`; `nh = 1080*0.05 = 54`. OK.

- [ ] **Step 4: Ejecutar los tests para ver que pasan**

Run: `cargo test thumbnail_gen`
Expected: PASS — 9 passed; 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src/core/thumbnail_gen.rs
git commit -m "feat: add pure thumbnail size/downscale helpers in thumbnail_gen"
```

---

## Task 2: `core/thumbnail_cache.rs` — `ThumbnailCache` thread-safe (TDD)

**Files:**
- Create: `src/core/thumbnail_cache.rs`
- Modify: `src/core/mod.rs` (registrar el módulo)

- [ ] **Step 1: Escribir los tests que fallan**

Crear `src/core/thumbnail_cache.rs`:

```rust
//! Cache en memoria de miniaturas, thread-safe.
//!
//! Sin evicción LRU: a `THUMB_MAX` (96px) cada miniatura es ~37 KB; cientos de
//! imágenes son decenas de MB, despreciables frente al límite de 512 MiB del
//! visor. `clear()` se llama al abrir una carpeta distinta.

use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use image::DynamicImage;

#[derive(Default)]
struct ThumbCacheInner {
    map: HashMap<PathBuf, DynamicImage>,
}

/// Cache de miniaturas en memoria (sin evicción). Thread-safe.
#[derive(Default)]
pub struct ThumbnailCache {
    inner: Mutex<ThumbCacheInner>,
}

impl ThumbnailCache {
    /// Crea un cache vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bloquea el mutex recuperándose de un lock envenenado (nunca panic).
    fn lock(&self) -> MutexGuard<'_, ThumbCacheInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Inserta (o reemplaza) la miniatura de `path`.
    pub fn insert(&self, path: PathBuf, image: DynamicImage) {
        self.lock().map.insert(path, image);
    }

    /// Devuelve la miniatura de `path`, o `None`.
    ///
    /// El `ThumbnailRef` mantiene el lock; se usa como `&DynamicImage` vía `Deref`.
    pub fn get(&self, path: &Path) -> Option<ThumbnailRef<'_>> {
        let guard = self.lock();
        if guard.map.contains_key(path) {
            Some(ThumbnailRef {
                guard,
                path: path.to_path_buf(),
            })
        } else {
            None
        }
    }

    /// `true` si `path` tiene miniatura.
    pub fn contains(&self, path: &Path) -> bool {
        self.lock().map.contains_key(path)
    }

    /// Vacía el cache (se llama al abrir una carpeta distinta).
    pub fn clear(&self) {
        self.lock().map.clear();
    }

    /// Número de miniaturas almacenadas.
    pub fn len(&self) -> usize {
        self.lock().map.len()
    }

    /// `true` si no hay miniaturas.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Acceso a una miniatura cacheada; mantiene el `MutexGuard` vivo.
///
/// El caller lo usa como `&DynamicImage` vía `Deref` (sin clonar pixels).
pub struct ThumbnailRef<'a> {
    guard: MutexGuard<'a, ThumbCacheInner>,
    path: PathBuf,
}

impl Deref for ThumbnailRef<'_> {
    type Target = DynamicImage;
    fn deref(&self) -> &DynamicImage {
        &self.guard.map[&self.path]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, RgbaImage};

    fn rgba(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn insert_then_get_roundtrips_dimensions() {
        let cache = ThumbnailCache::new();
        cache.insert(PathBuf::from("a.png"), rgba(96, 54));
        let got = cache
            .get(Path::new("a.png"))
            .expect("debería estar cacheada");
        assert_eq!(got.dimensions(), (96, 54));
    }

    #[test]
    fn get_on_missing_path_returns_none() {
        let cache = ThumbnailCache::new();
        assert!(cache.get(Path::new("nope.png")).is_none());
    }

    #[test]
    fn contains_reflects_state() {
        let cache = ThumbnailCache::new();
        assert!(!cache.contains(Path::new("a.png")));
        cache.insert(PathBuf::from("a.png"), rgba(96, 54));
        assert!(cache.contains(Path::new("a.png")));
    }

    #[test]
    fn clear_empties_the_cache() {
        let cache = ThumbnailCache::new();
        cache.insert(PathBuf::from("a.png"), rgba(96, 54));
        cache.insert(PathBuf::from("b.png"), rgba(96, 54));
        assert_eq!(cache.len(), 2);
        assert!(!cache.is_empty());
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert!(cache.get(Path::new("a.png")).is_none());
    }

    #[test]
    fn overwrite_same_key_replaces_entry() {
        let cache = ThumbnailCache::new();
        cache.insert(PathBuf::from("a.png"), rgba(96, 54));
        cache.insert(PathBuf::from("a.png"), rgba(50, 30));
        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache.get(Path::new("a.png")).expect("cacheada").dimensions(),
            (50, 30)
        );
    }
}
```

- [ ] **Step 2: Registrar el módulo y ver que los tests fallan**

Modificar `src/core/mod.rs` añadiendo la línea (mantener orden alfabético, entre `preload` y `thumbnail_gen`):

```rust
pub mod thumbnail_cache;
```

Run: `cargo test thumbnail_cache`
Expected: FAIL — `error[E0432]: unresolved import` (el módulo no existe todavía).

- [ ] **Step 3: Verificación de compilación**

El código del Step 1 ya incluye la implementación. Nota: `#[derive(Default)]` en `ThumbnailCache` funciona porque `Mutex<T>: Default` cuando `T: Default`.

- [ ] **Step 4: Ejecutar los tests para ver que pasan**

Run: `cargo test thumbnail_cache`
Expected: PASS — 5 passed; 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src/core/thumbnail_cache.rs src/core/mod.rs
git commit -m "feat: add in-memory ThumbnailCache with thread-safe insert/get/clear"
```

---

## Task 3: `ui/sidebar.rs` — `SidebarState` con grid, highlight, click y placeholder

**Files:**
- Modify: `src/ui/sidebar.rs` (reescribir el stub de 1 línea)

- [ ] **Step 1: Escribir la implementación**

Reemplazar TODO el contenido actual de `src/ui/sidebar.rs` por:

```rust
//! Panel lateral con miniaturas de la carpeta.
//!
//! Solo presenta: lee el `ThumbnailCache` (core), construye texturas de egui y
//! devuelve el índice seleccionado. La carga/navegación la orquesta `app.rs`.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use egui::{Color32, Sense, Stroke, StrokeKind};

use crate::core::navigation::Navigation;
use crate::core::thumbnail_cache::ThumbnailCache;

/// Tamaño de celda del grid (lado del cuadrado, incluye el margen visual).
const CELL_SIZE: f32 = 108.0;

/// `true` si el panel se muestra.
#[derive(Debug)]
pub struct SidebarState {
    pub show: bool,
    /// Texturas GPU por path; se construyen al aparecer la miniatura en el cache.
    textures: HashMap<PathBuf, egui::TextureHandle>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarState {
    /// Crea el estado del sidebar visible por defecto.
    pub fn new() -> Self {
        Self {
            show: true,
            textures: HashMap::new(),
        }
    }

    /// Libera las texturas (se llama al abrir una carpeta distinta).
    pub fn clear_textures(&mut self) {
        self.textures.clear();
    }

    /// Pinta el panel lateral y devuelve el índice de la miniatura clickeada,
    /// o `None` si no hubo click.
    ///
    /// # Arguments
    /// * `ctx` - Contexto de egui (para `SidePanel` y `load_texture`).
    /// * `nav` - Navegación actual; su lista define las celdas del grid.
    /// * `thumb_cache` - Cache de miniaturas (core) del que se leen los datos.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        nav: &Navigation,
        thumb_cache: &ThumbnailCache,
    ) -> Option<usize> {
        let mut selection = None;
        egui::SidePanel::left("sidebar")
            .min_width(CELL_SIZE + 20.0)
            .default_width(CELL_SIZE + 40.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (i, path) in nav.images.iter().enumerate() {
                            if !self.textures.contains_key(path) {
                                if let Some(thumb) = thumb_cache.get(path) {
                                    let tex = load_thumbnail(ctx, &thumb);
                                    self.textures.insert(path.clone(), tex);
                                }
                            }
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(CELL_SIZE, CELL_SIZE), Sense::click());
                            if response.clicked() {
                                selection = Some(i);
                            }
                            if i == nav.current {
                                ui.painter().rect_filled(
                                    rect,
                                    4.0,
                                    ui.visuals().selection.bg_fill,
                                );
                            }
                            match self.textures.get(path) {
                                Some(tex) => {
                                    let fit = fit_thumbnail(rect.size(), tex.size());
                                    let top_left =
                                        rect.center() - egui::vec2(fit.x, fit.y) * 0.5;
                                    let img_rect = egui::Rect::from_min_size(top_left, fit);
                                    ui.painter().image(
                                        tex.id(),
                                        img_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                }
                                None => {
                                    ui.painter().rect_filled(rect, 4.0, Color32::from_gray(60));
                                }
                            }
                            if response.hovered() {
                                ui.painter().rect_stroke(
                                    rect,
                                    4.0,
                                    Stroke::new(1.0, Color32::from_gray(180)),
                                    StrokeKind::Inside,
                                );
                            }
                        }
                    });
                });
            });
        selection
    }
}

/// Carga la miniatura como textura de egui.
fn load_thumbnail(ctx: &egui::Context, thumb: &image::DynamicImage) -> egui::TextureHandle {
    let size = [thumb.width() as usize, thumb.height() as usize];
    let rgba = thumb.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture("thumbnail", color_image, egui::TextureOptions::LINEAR)
}

/// Devuelve el tamaño de la imagen ajustado dentro de `avail` manteniendo el
/// aspect ratio y centrado; nunca amplía por encima del tamaño natural.
fn fit_thumbnail(avail: egui::Vec2, img: egui::Vec2) -> egui::Vec2 {
    if img.x <= 0.0 || img.y <= 0.0 {
        return avail;
    }
    let scale = (avail.x / img.x).min(avail.y / img.y).min(1.0);
    egui::vec2(img.x * scale, img.y * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_vec2(a: egui::Vec2, b: egui::Vec2) {
        assert!((a.x - b.x).abs() < 1e-3, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < 1e-3, "y: {} != {}", a.y, b.y);
    }

    #[test]
    fn fit_square_image_into_square_cell_fills_cell() {
        assert_vec2(fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(96.0, 96.0)), egui::vec2(96.0, 96.0));
    }

    #[test]
    fn fit_wide_image_preserves_aspect_ratio() {
        // img 96x54 en celda 108x108 → escala por alto 108/54 = 2 → no upscale (min 1.0)
        assert_vec2(fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(96.0, 54.0)), egui::vec2(96.0, 54.0));
    }

    #[test]
    fn fit_tall_image_preserves_aspect_ratio() {
        assert_vec2(fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(54.0, 96.0)), egui::vec2(54.0, 96.0));
    }

    #[test]
    fn fit_never_upscales_beyond_natural_size() {
        assert_vec2(fit_thumbnail(egui::vec2(200.0, 200.0), egui::vec2(50.0, 50.0)), egui::vec2(50.0, 50.0));
    }

    #[test]
    fn fit_scales_down_when_avail_is_smaller() {
        assert_vec2(fit_thumbnail(egui::vec2(40.0, 20.0), egui::vec2(96.0, 54.0)), egui::vec2(35.56, 20.0));
    }

    #[test]
    fn fit_zero_size_returns_avail() {
        assert_vec2(fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(0.0, 0.0)), egui::vec2(108.0, 108.0));
    }
}
```

- [ ] **Step 2: Ejecutar los tests del módulo**

Run: `cargo test sidebar`
Expected: PASS — 6 tests de `fit_thumbnail` en verde. (Los tests de `theme`/`toast` también pasan.)

- [ ] **Step 3: Verificar con clippy y fmt**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS — 0 warnings.

Run: `cargo fmt --check` (si falla, ejecutar `cargo fmt` y re-ejecutar `cargo clippy`).

- [ ] **Step 4: Commit**

```bash
git add src/ui/sidebar.rs
git commit -m "feat: add sidebar with thumbnail grid, highlight and click-navigation"
```

---

## Task 4: `app.rs` — pipeline de miniaturas (pool + canales) e integración del sidebar

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Añadir imports, constantes y campos**

1. En el bloque de `use` actual (líneas 3-18), añadir los imports de thumbnail. El bloque queda:

```rust
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, MutexGuard};

use eframe::egui;
use image::{DynamicImage, GenericImageView};

use crate::config::settings::Settings;
use crate::core::image_cache::ImageCache;
use crate::core::image_loader::load_image;
use crate::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use crate::core::preload::{preload_targets, PRELOAD_DEPTH};
use crate::core::thumbnail_cache::ThumbnailCache;
use crate::core::thumbnail_gen::{generate_thumbnail, THUMB_MAX};
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{sidebar::SidebarState, theme, toast::Toasts, viewer};
use crate::utils::errors::Result;
use crate::utils::paths::settings_path;
```

2. Tras la definición de `LoadEvent` (línea 27), añadir la constante del pool:

```rust
/// Número de workers del pool de miniaturas (acotado, nunca un thread por imagen).
const THUMB_POOL_SIZE: usize = 3;
```

3. Añadir los campos nuevos al struct `ShImagesApp` (tras `last_applied: Option<PathBuf>` en línea 53):

```rust
    /// Cache en memoria de miniaturas, compartido con el pool de workers.
    thumb_cache: Arc<ThumbnailCache>,
    /// Emisor del canal de paths a miniaturizar (la UI encola, los workers consumen).
    thumb_tx: mpsc::Sender<PathBuf>,
    /// Receptor de notificaciones de "miniatura lista" (solo dispara repaint).
    thumb_events_rx: Option<mpsc::Receiver<()>>,
    /// Estado del sidebar (visible + texturas GPU).
    sidebar: SidebarState,
```

- [ ] **Step 2: Inicializar el pool en `new()`**

Modificar el cuerpo de `new()` (líneas 61-86). El bloque actual:

```rust
        let cache = Arc::new(ImageCache::new(settings.cache_memory_limit_mb));
        let (tx, rx) = mpsc::channel();
        Self {
            settings,
            ctx: cc.egui_ctx.clone(),
            navigation: None,
            transform: ViewTransform::new(Vec2::ZERO, Vec2::ZERO),
            texture: None,
            cache,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            tx,
            rx: Some(rx),
            toasts: Toasts::new(),
            user_interacted: false,
            last_viewport: None,
            last_applied: None,
        }
```

pasa a:

```rust
        let cache = Arc::new(ImageCache::new(settings.cache_memory_limit_mb));
        let (tx, rx) = mpsc::channel();
        let thumb_cache = Arc::new(ThumbnailCache::new());
        let (thumb_tx, thumb_rx) = mpsc::channel::<PathBuf>();
        let thumb_rx = Arc::new(thumb_rx);
        let (thumb_events_tx, thumb_events_rx) = mpsc::channel::<()>();
        for _ in 0..THUMB_POOL_SIZE {
            let rx = thumb_rx.clone();
            let cache = thumb_cache.clone();
            let events_tx = thumb_events_tx.clone();
            std::thread::spawn(move || {
                while let Ok(path) = rx.recv() {
                    let result = load_image(&path).map(|image| {
                        let thumb = generate_thumbnail(&image, THUMB_MAX);
                        cache.insert(path.clone(), thumb);
                    });
                    if let Err(e) = &result {
                        tracing::debug!(error = %e, path = %path.display(), "thumbnail failed");
                    }
                    if events_tx.send(()).is_err() {
                        tracing::debug!("thumbnail event dropped (receiver gone)");
                    }
                }
            });
        }
        Self {
            settings,
            ctx: cc.egui_ctx.clone(),
            navigation: None,
            transform: ViewTransform::new(Vec2::ZERO, Vec2::ZERO),
            texture: None,
            cache,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            tx,
            rx: Some(rx),
            toasts: Toasts::new(),
            user_interacted: false,
            last_viewport: None,
            last_applied: None,
            thumb_cache,
            thumb_tx,
            thumb_events_rx: Some(thumb_events_rx),
            sidebar: SidebarState::new(),
        }
```

> Nota de concurrencia: `Arc<mpsc::Receiver<PathBuf>>` se comparte entre los 3 workers; `Receiver<T>: Send + Sync` cuando `T: Send`, y `PathBuf: Send`, así que `recv()` por referencia compartida es seguro. Cada mensaje despierta un único worker.

- [ ] **Step 3: Encolar miniaturas y limpiar al abrir carpeta**

Modificar `open_path` (líneas 117-130). El bloque actual:

```rust
        match Navigation::from_folder(&path, SUPPORTED_EXTENSIONS) {
            Ok(nav) => {
                tracing::info!(path = %path.display(), "opening image");
                self.navigation = Some(nav);
                self.start_load(path);
            }
```

pasa a:

```rust
        match Navigation::from_folder(&path, SUPPORTED_EXTENSIONS) {
            Ok(nav) => {
                tracing::info!(path = %path.display(), "opening image");
                self.thumb_cache.clear();
                self.sidebar.clear_textures();
                for image_path in &nav.images {
                    if self.thumb_tx.send(image_path.clone()).is_err() {
                        tracing::debug!("thumbnail queue closed; workers gone");
                        break;
                    }
                }
                self.navigation = Some(nav);
                self.start_load(path);
            }
```

- [ ] **Step 4: Añadir `poll_thumbnails`, `navigate_to` y toggle del sidebar**

Añadir estos métodos en `impl ShImagesApp`, tras `poll_loader` (después de la línea 262):

```rust
    /// Drena las notificaciones de miniaturas y dispara un repaint si hubo.
    ///
    /// La UI no necesita el contenido del evento: lee `thumb_cache` directamente
    /// en el frame siguiente.
    fn poll_thumbnails(&mut self) {
        let Some(rx) = self.thumb_events_rx.take() else { return };
        let mut repaint = false;
        while rx.try_recv().is_ok() {
            repaint = true;
        }
        self.thumb_events_rx = Some(rx);
        if repaint {
            self.ctx.request_repaint();
        }
    }

    /// Salta a la imagen `index` de la carpeta (click en una miniatura).
    fn navigate_to(&mut self, index: usize) {
        let Some(nav) = &mut self.navigation else {
            return;
        };
        if index >= nav.images.len() {
            return;
        }
        nav.current = index;
        if let Some(path) = nav.current_path().cloned() {
            self.start_load(path);
        }
    }

    /// Alterna la visibilidad del sidebar.
    fn toggle_sidebar(&mut self) {
        self.sidebar.show = !self.sidebar.show;
    }
```

- [ ] **Step 5: Renderizar el sidebar y añadir el atajo `H`**

1. En `handle_shortcuts` (líneas 294-312), añadir tras el bloque de `fit`:

```rust
        let toggle_side = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::H));
        if toggle_side {
            self.toggle_sidebar();
        }
```

2. En el método `ui` (líneas 323-372), insertar el `SidePanel` **antes** del `egui::CentralPanel` (tras la llamada a `self.poll_loader(t)` y antes de `let mut want_open = false;`):

```rust
        self.poll_loader(t);
        self.poll_thumbnails();

        if self.sidebar.show {
            if let Some(nav) = &self.navigation {
                let selected = self.sidebar.show(ui.ctx(), nav, &self.thumb_cache);
                if let Some(index) = selected {
                    self.navigate_to(index);
                }
            }
        }

        let mut want_open = false;
```

> Nota de borrows: `self.sidebar.show(ui.ctx(), nav, &self.thumb_cache)` presta `&mut self.sidebar`, `&self.navigation` (vía `nav`) y `&self.thumb_cache` — campos disjuntos, el borrow checker lo acepta. El `selected` se consume después del bloque `if let`, cuando los borrows ya terminaron.

- [ ] **Step 6: Verificación completa**

Run: `cargo check --all-targets`
Expected: PASS.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS — 0 warnings.

Run: `cargo fmt --check` (si falla, `cargo fmt`).

Run: `cargo test`
Expected: PASS — 95 tests (75 previos + 9 `thumbnail_gen` + 5 `thumbnail_cache` + 6 `sidebar`).

- [ ] **Step 7: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire thumbnail pipeline and sidebar into the app"
```

---

## Task 5: QA final AGENTS.md

**Files:** ninguno (solo verificación)

- [ ] **Step 1: Suite completa en debug y release**

Run: `cargo check` — PASS.
Run: `cargo clippy --all-targets -- -D warnings` — PASS.
Run: `cargo fmt --check` — PASS.
Run: `cargo test` — 95 passed.
Run: `cargo test --release` — 95 passed.

- [ ] **Step 2: Auditoría de seguridad (AGENTS.md §2.1, §7.1)**

Run: `rg -n "unwrap\(|expect\(|unimplemented!|todo!|panic!" src --glob '!**/tests/**'`
Expected: solo apariciones dentro de bloques `#[cfg(test)]`. En producción: ninguna.

Run: `rg -n "println!|eprintln!" src`
Expected: sin resultados (todo logging es `tracing`).

- [ ] **Step 3: `core/` sin egui**

Run: `rg -n "egui|eframe" src/core`
Expected: solo en doc-comments (p.ej. menciones a AGENTS.md), sin `use egui`.

- [ ] **Step 4: Sin código muerto nuevo**

Verificar que `fit_thumbnail`, `SidebarState::clear_textures`, `navigate_to`, `poll_thumbnails` y `toggle_sidebar` se usan desde el código (grep rápido). `THUMB_MAX` y `generate_thumbnail` se usan en `app.rs`; `thumbnail_size` se usa dentro de `generate_thumbnail`.

- [ ] **Step 5: Verificación manual (documentar en PR)**

1. Abrir carpeta con imágenes → el sidebar muestra un grid de miniaturas que se rellena progresivamente.
2. La imagen actual aparece resaltada en el grid.
3. Click en una miniatura distinta → el visor salta a esa imagen.
4. Pulsar `H` → el sidebar se oculta; `H` de nuevo → reaparece.
5. Navegar con `←`/`→` → la imagen siguiente/anterior se ve sin espera perceptible (no regresión de S2).
6. Abrir una carpeta distinta → el sidebar se regenera (sin miniaturas de la carpeta anterior).
7. Carpeta con un archivo corrupto → su celda muestra placeholder gris; la app no crashea.

---

## Self-Review del plan vs spec

**1. Cobertura del spec:**
- `thumbnail_size`/`generate_thumbnail` (spec §4) → Task 1. ✓
- `ThumbnailCache` insert/get/contains/clear/len/is_empty (spec §4) → Task 2. ✓
- Pipeline en `app.rs`: canal de paths + pool 3 workers + `poll_thumbnails` + `open_path` encola + `clear()` (spec §4) → Task 4. ✓
- `ui/sidebar.rs`: grid, highlight, click, placeholder, toggle (spec §4) → Tasks 3-4. ✓
- Tests: thumbnail_gen (9), thumbnail_cache (5), sidebar helpers (6) (spec §6) → Tasks 1-3. ✓
- Criterios de éxito (spec §7): check/clippy/fmt/test/release, sin unwrap/expect, core sin egui, no regresión S2 → Task 5. ✓
- `core/sidebar_layout.rs` (spec, opcional) → **deliberadamente no se crea** (código muerto; ver nota en File Structure). Documentado.

**2. Placeholder scan:** sin "TBD"/"TODO"; cada paso incluye código completo o comandos exactos.

**3. Type consistency:** 
- `ThumbnailCache` se usa en `app.rs` (`thumb_cache: Arc<ThumbnailCache>`) y en `sidebar.rs::show(..., thumb_cache: &ThumbnailCache)`. ✓
- `SidebarState::show` devuelve `Option<usize>`; `navigate_to(usize)` lo consume. ✓
- `THUMB_MAX` (u32) se usa en `generate_thumbnail(&image, THUMB_MAX)` y `thumbnail_size(_, _, THUMB_MAX)` — mismos tipos. ✓
- `fit_thumbnail(egui::Vec2, egui::Vec2) -> egui::Vec2` usada con `rect.size()` y `tex.size()` (ambos `egui::Vec2`). ✓
- `clear_textures()` y `thumb_cache.clear()` se llaman juntos en `open_path`. ✓
