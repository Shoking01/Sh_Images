# Fase 1 — Visor Básico Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mostrar imágenes reales con zoom/pan/fit, navegación circular por carpeta, apertura con diálogo nativo, carga asíncrona mínima (thread worker) y toasts de error.

**Architecture:** `core/view.rs` (math puro de transformación, sin egui), `core/navigation.rs` (lista ordenada de imágenes de la carpeta, next/prev circular), `ui/viewer.rs` (painter egui que pinta la textura con la transformación y captura inputs), `ui/toast.rs` (overlay de errores), `app.rs` como glue: estado, thread worker + canal `std::mpsc`, menú, atajos.

**Tech Stack:** `rfd` 0.15 (diálogo nativo), egui/eframe 0.35, `image` 0.25. Sin otras dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-07-31-fase1-visor-basico-design.md`

---

## File Structure

```
Cargo.toml                       # Modify — añadir rfd
src/core/mod.rs                  # Modify — añadir pub mod view;
src/core/view.rs                 # Create (Task 1) — Vec2 + ViewTransform (math puro)
src/core/navigation.rs           # Rewrite (Task 2) — reemplaza el stub de Fase 0
src/ui/mod.rs                    # Modify — añadir pub mod toast;
src/ui/viewer.rs                 # Rewrite (Task 5) — painter real
src/ui/toast.rs                  # Create (Task 4) — toasts
src/app.rs                       # Rewrite (Task 6) — glue completo
tests/fixtures/sample.jpg        # Create (Task 3) — JPEG 16x16
```

> Nota: el `Vec2`/`Point2` propio de `core/view.rs` evita que `core/` dependa de egui (AGENTS.md §3.2). En los módulos UI se convierte a `egui::Vec2` en el límite de módulo.

---

## Task 1: `core/view.rs` — ViewTransform (math puro)

**Files:**
- Modify: `src/core/mod.rs`
- Create: `src/core/view.rs`

- [ ] **Step 1: Declarar el módulo**

Añadir `pub mod view;` a `src/core/mod.rs` (orden alfabético, tras `navigation`):

```rust
pub mod view;
```

- [ ] **Step 2: Escribir los tests que fallan**

Crear `src/core/view.rs`:

```rust
//! Matemática pura de zoom/pan/fit para el visor.
//!
//! `core/` no depende de `egui`; este módulo define un vector 2D mínimo propio.

use std::ops::{Add, Div, Mul, Sub};

/// Vector/posición 2D mínimo del módulo core.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x / rhs, self.y / rhs)
    }
}

/// Zoom máximo como múltiplo del tamaño fit (8x el fit).
pub const MAX_ZOOM: f32 = 8.0;

/// Transformación de vista: escala y desplazamiento de la imagen en el canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    pub zoom: f32,
    pub pan: Vec2,
    pub image_size: Vec2,
    pub viewport: Vec2,
}

impl ViewTransform {
    /// Crea una transformación en fit inicial.
    pub fn new(image_size: Vec2, viewport: Vec2) -> Self {
        let mut t = Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            image_size,
            viewport,
        };
        t.fit();
        t
    }

    /// Zoom que hace caber la imagen completa en el viewport.
    pub fn fit_zoom(&self) -> f32 {
        if self.image_size.x <= 0.0
            || self.image_size.y <= 0.0
            || self.viewport.x <= 0.0
            || self.viewport.y <= 0.0
        {
            return 1.0;
        }
        let zx = self.viewport.x / self.image_size.x;
        let zy = self.viewport.y / self.image_size.y;
        zx.min(zy)
    }

    /// Esquina superior izquierda de la imagen en coordenadas de pantalla.
    pub fn image_origin_screen(&self) -> Vec2 {
        let center = self.viewport.mul(0.5);
        let half = self.image_size.mul(self.zoom * 0.5);
        center.sub(half).add(self.pan)
    }

    /// Ajusta a fit completo: zoom = fit, pan = centrado.
    pub fn fit(&mut self) {
        self.zoom = self.fit_zoom();
        self.pan = Vec2::ZERO;
    }

    /// Cambia el zoom por `factor` manteniendo fijo el punto de la imagen bajo `anchor`.
    pub fn apply_zoom_at(&mut self, anchor: Vec2, factor: f32) {
        let fit = self.fit_zoom();
        let min = fit;
        let max = fit * MAX_ZOOM;
        let new_zoom = (self.zoom * factor).clamp(min, max);
        if (new_zoom - self.zoom).abs() < f32::EPSILON {
            return;
        }
        let origin = self.image_origin_screen();
        let image_point = anchor.sub(origin).div(self.zoom);
        let new_origin = anchor.sub(image_point.mul(new_zoom));
        let center = self.viewport.mul(0.5);
        self.pan = new_origin.sub(center).add(self.image_size.mul(new_zoom * 0.5));
        self.zoom = new_zoom;
    }

    /// Desplaza el pan libremente (sin clamp).
    pub fn pan_by(&mut self, delta: Vec2) {
        self.pan = self.pan.add(delta);
    }

    /// Actualiza el tamaño del canvas.
    pub fn set_viewport(&mut self, viewport: Vec2) {
        self.viewport = viewport;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn fit_zoom_scales_to_fit_wide_image() {
        let t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
        assert!(approx(t.fit_zoom(), 0.25)); // min(500/2000, 500/1000)
    }

    #[test]
    fn fit_zoom_scales_to_fit_tall_image() {
        let t = ViewTransform::new(Vec2::new(1000.0, 2000.0), Vec2::new(500.0, 500.0));
        assert!(approx(t.fit_zoom(), 0.25)); // min(500/1000, 500/2000)
    }

    #[test]
    fn fit_zoom_returns_1_on_zero_dimension() {
        let t = ViewTransform::new(Vec2::ZERO, Vec2::new(500.0, 500.0));
        assert_eq!(t.fit_zoom(), 1.0);
    }

    #[test]
    fn apply_zoom_at_keeps_anchor_point_fixed() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let anchor = Vec2::new(150.0, 200.0);
        let origin = t.image_origin_screen();
        let image_point = anchor.sub(origin).div(t.zoom);
        t.apply_zoom_at(anchor, 1.5);
        let new_origin = t.image_origin_screen();
        let new_screen = new_origin.add(image_point.mul(t.zoom));
        assert!(approx(new_screen.x, anchor.x));
        assert!(approx(new_screen.y, anchor.y));
    }

    #[test]
    fn apply_zoom_at_clamps_to_max_zoom() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 1000.0);
        assert!(approx(t.zoom, fit * MAX_ZOOM));
    }

    #[test]
    fn apply_zoom_at_clamps_to_min_fit() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 0.0001);
        assert!(approx(t.zoom, fit));
    }

    #[test]
    fn pan_by_moves_image_by_delta() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let before = t.image_origin_screen();
        t.pan_by(Vec2::new(10.0, -20.0));
        let after = t.image_origin_screen();
        assert!(approx(after.x - before.x, 10.0));
        assert!(approx(after.y - before.y, -20.0));
    }

    #[test]
    fn fit_resets_to_centered_initial() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        t.apply_zoom_at(Vec2::new(100.0, 100.0), 2.0);
        t.pan_by(Vec2::new(50.0, 50.0));
        t.fit();
        let expected = t.fit_zoom();
        assert!(approx(t.zoom, expected));
        // En fit con pan 0, la imagen queda centrada.
        let expected_origin = Vec2::new(
            (500.0 - 1000.0 * expected) / 2.0,
            (500.0 - 1000.0 * expected) / 2.0,
        );
        let origin = t.image_origin_screen();
        assert!(approx(origin.x, expected_origin.x));
        assert!(approx(origin.y, expected_origin.y));
    }
}
```

- [ ] **Step 3: Ejecutar los tests para verificar que fallan**

Run: `cargo test --lib core::view`
Expected: FAIL — `view` no compila (módulo no declarado).

- [ ] **Step 4: Ejecutar los tests para verificar que pasan**

El código de Step 2 es la implementación completa. Run: `cargo test --lib core::view`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add src/core/mod.rs src/core/view.rs
git commit -m "feat: add pure view transform math for zoom/pan/fit"
```

---

## Task 2: `core/navigation.rs` — Navegación real

**Files:**
- Rewrite: `src/core/navigation.rs`

- [ ] **Step 1: Escribir los tests que fallan**

Reemplazar `src/core/navigation.rs`:

```rust
//! Navegación entre imágenes de una carpeta.

use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::errors::{Result, ShImagesError};

/// Extensiones de imagen soportadas (sin punto; el filtro es case-insensitive).
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "avif",
];

/// Estado de navegación sobre la lista ordenada de imágenes de una carpeta.
pub struct Navigation {
    /// Rutas absolutas de las imágenes, ordenadas alfabéticamente.
    pub images: Vec<PathBuf>,
    /// Índice de la imagen actual.
    pub current: usize,
}

impl Navigation {
    /// Crea la navegación sobre la carpeta del archivo `image_path`.
    ///
    /// Lee el directorio padre de `image_path`, filtra por extensiones
    /// soportadas (case-insensitive), ordena alfabéticamente y localiza el
    /// índice de `image_path` (o 0 si no está).
    ///
    /// # Errors
    /// * `ShImagesError::Io` si el directorio no existe o no puede leerse.
    /// * `ShImagesError::Config` si `image_path` no tiene directorio padre.
    pub fn from_folder(image_path: &Path, supported_exts: &[&str]) -> Result<Self> {
        let folder = image_path.parent().ok_or_else(|| {
            ShImagesError::Config("image path has no parent directory".to_string())
        })?;
        let mut images = Vec::new();
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && has_supported_extension(&path, supported_exts) {
                images.push(path);
            }
        }
        images.sort();
        let current = images.iter().position(|p| p == image_path).unwrap_or(0);
        Ok(Self { images, current })
    }

    /// Avanza a la siguiente imagen; al final de la lista vuelve al inicio.
    pub fn next(&mut self) {
        if self.images.is_empty() {
            return;
        }
        self.current = (self.current + 1) % self.images.len();
    }

    /// Retrocede a la imagen anterior; al inicio de la lista va al final.
    pub fn prev(&mut self) {
        if self.images.is_empty() {
            return;
        }
        self.current = (self.current + self.images.len() - 1) % self.images.len();
    }

    /// Ruta de la imagen actual, o `None` si la lista está vacía.
    pub fn current_path(&self) -> Option<&PathBuf> {
        self.images.get(self.current)
    }
}

/// Devuelve `true` si `path` tiene una extensión en `supported_exts`.
pub fn has_supported_extension(path: &Path, supported_exts: &[&str]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| supported_exts.iter().any(|s| s.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_folder() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let folder = dir.path().to_path_buf();
        for name in ["b.png", "a.jpg", "c.txt", "d.JPG"] {
            fs::write(folder.join(name), b"x").unwrap();
        }
        (dir, folder)
    }

    #[test]
    fn from_folder_filters_by_extension() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        assert_eq!(nav.images.len(), 3); // b.png, a.jpg, d.JPG (c.txt excluido)
        for p in &nav.images {
            assert!(has_supported_extension(p, SUPPORTED_EXTENSIONS));
        }
    }

    #[test]
    fn from_folder_sorts_alphabetically() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        let names: Vec<String> = nav
            .images
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.jpg", "b.png", "d.JPG"]);
    }

    #[test]
    fn from_folder_sets_current_to_matching_path() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("d.JPG"), SUPPORTED_EXTENSIONS).unwrap();
        assert_eq!(nav.current, 2);
    }

    #[test]
    fn from_folder_falls_back_to_zero_if_not_found() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("zzz.png"), SUPPORTED_EXTENSIONS).unwrap();
        assert_eq!(nav.current, 0);
    }

    #[test]
    fn from_folder_returns_io_error_on_missing_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("no_such_dir").join("a.png");
        let err = Navigation::from_folder(&missing, SUPPORTED_EXTENSIONS).unwrap_err();
        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn from_folder_returns_config_error_when_no_parent() {
        let err = Navigation::from_folder(Path::new("a.png"), SUPPORTED_EXTENSIONS).unwrap_err();
        assert!(matches!(err, ShImagesError::Config(_)));
    }

    #[test]
    fn next_wraps_circularly() {
        let (_d, folder) = setup_folder();
        let mut nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        nav.next();
        assert_eq!(nav.current, 1);
        nav.next();
        assert_eq!(nav.current, 2);
        nav.next();
        assert_eq!(nav.current, 0);
    }

    #[test]
    fn prev_wraps_circularly() {
        let (_d, folder) = setup_folder();
        let mut nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        nav.prev();
        assert_eq!(nav.current, 2);
    }

    #[test]
    fn next_on_empty_is_noop() {
        let dir = tempdir().unwrap();
        let empty = dir.path().join("nothing.png");
        let mut nav = Navigation::from_folder(&empty, SUPPORTED_EXTENSIONS).unwrap();
        assert!(nav.images.is_empty());
        nav.next();
        nav.prev();
        assert_eq!(nav.current, 0);
        assert!(nav.current_path().is_none());
    }
}
```

- [ ] **Step 2: Ejecutar los tests para verificar que fallan**

Run: `cargo test --lib core::navigation`
Expected: FAIL — el stub de Fase 0 no tiene `from_folder`.

- [ ] **Step 3: Ejecutar los tests para verificar que pasan**

El código de Step 1 es la implementación completa. Run: `cargo test --lib core::navigation`
Expected: PASS (9 tests).

- [ ] **Step 4: Commit**

```bash
git add src/core/navigation.rs
git commit -m "feat: add folder navigation with extension filter and circular wrap"
```

---

## Task 3: Fixture JPEG + test multi-formato en `image_loader`

**Files:**
- Create: `tests/fixtures/sample.jpg`
- Modify: `src/core/image_loader.rs`

- [ ] **Step 1: Crear el fixture JPEG 16x16**

Run (PowerShell, genera un JPEG rojo 16x16 con System.Drawing):

```powershell
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 16,16
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::Red)
$bmp.Save("tests\fixtures\sample.jpg", [System.Drawing.Imaging.ImageFormat]::Jpeg)
$g.Dispose(); $bmp.Dispose()
```

Verify: `Get-Item tests\fixtures\sample.jpg` → tamaño < 100KB.

- [ ] **Step 2: Añadir test de JPEG a `image_loader.rs`**

En el bloque `#[cfg(test)]` de `src/core/image_loader.rs`, añadir después del test de PNG:

```rust
    #[test]
    fn decoding_valid_jpeg_returns_image() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.jpg");
        let img = load_image(&path).unwrap();
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
    }
```

- [ ] **Step 3: Ejecutar los tests**

Run: `cargo test --lib core::image_loader`
Expected: PASS (6 tests: 5 previos + JPEG).

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/sample.jpg src/core/image_loader.rs
git commit -m "test: cover jpeg decoding with 16x16 fixture"
```

---

## Task 4: `ui/toast.rs` — Toasts

**Files:**
- Modify: `src/ui/mod.rs`
- Create: `src/ui/toast.rs`

- [ ] **Step 1: Declarar el módulo**

Añadir `pub mod toast;` a `src/ui/mod.rs` (orden alfabético):

```rust
pub mod toast;
```

- [ ] **Step 2: Escribir los tests que fallan**

Crear `src/ui/toast.rs`:

```rust
//! Overlay de notificaciones (toasts) dibujado con egui.

use eframe::egui;

/// Duración de un toast en segundos.
pub const TOAST_SECONDS: f64 = 3.0;

struct Toast {
    message: String,
    expires_at: f64,
}

/// Colección de toasts visibles.
#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    /// Crea una colección vacía.
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade un toast que expira `TOAST_SECONDS` después de `now` (segundos de egui).
    pub fn push(&mut self, message: impl Into<String>, now: f64) {
        self.items.push(Toast {
            message: message.into(),
            expires_at: now + TOAST_SECONDS,
        });
    }

    /// Elimina los toasts expirados en `now`.
    pub fn update(&mut self, now: f64) {
        self.items.retain(|t| t.expires_at > now);
    }

    /// Devuelve `true` si no hay toasts visibles.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Dibuja los toasts en la esquina inferior derecha.
    pub fn show(&self, ui: &mut egui::Ui) {
        if self.items.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                for toast in &self.items {
                    ui.label(&toast.message);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_adds_toast_with_expiry() {
        let mut toasts = Toasts::new();
        toasts.push("test", 10.0);
        assert_eq!(toasts.items.len(), 1);
        assert_eq!(toasts.items[0].message, "test");
        assert!((toasts.items[0].expires_at - 13.0).abs() < 1e-9);
    }

    #[test]
    fn update_removes_expired_toasts() {
        let mut toasts = Toasts::new();
        toasts.push("one", 10.0);
        toasts.push("two", 12.0);
        toasts.update(13.1);
        assert!(toasts.is_empty());
    }

    #[test]
    fn update_keeps_unexpired_toasts() {
        let mut toasts = Toasts::new();
        toasts.push("one", 10.0);
        toasts.push("two", 14.0);
        toasts.update(12.9);
        assert_eq!(toasts.items.len(), 1);
        assert_eq!(toasts.items[0].message, "two");
    }
}
```

- [ ] **Step 3: Ejecutar los tests para verificar que fallan**

Run: `cargo test --lib ui::toast`
Expected: FAIL — `toast` no compila (módulo no declarado o sin `show`).

> El test `push_adds_toast_with_expiry` accede a `toasts.items` (privado); como está en el mismo módulo via `use super::*`, es válido.

- [ ] **Step 4: Ejecutar los tests para verificar que pasan**

Run: `cargo test --lib ui::toast`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/ui/mod.rs src/ui/toast.rs
git commit -m "feat: add toast overlay notifications"
```

---

## Task 5: `ui/viewer.rs` — Painter de la imagen

**Files:**
- Rewrite: `src/ui/viewer.rs`

- [ ] **Step 1: Escribir el painter**

Reemplazar `src/ui/viewer.rs`:

```rust
//! Componente que pinta la imagen con la transformación de vista.
//!
//! Solo presenta: dibuja la textura con `ViewTransform` y reporta los inputs.
//! Toda la lógica de transformación vive en `core::view`.

use eframe::egui;

use crate::core::view::{Vec2, ViewTransform};

/// Resultado de interacción del visor en un frame.
#[derive(Debug, Default)]
pub struct ViewResponse {
    /// El usuario hizo zoom con la rueda.
    pub zoomed: bool,
    /// El usuario arrastró (pan).
    pub panned: bool,
}

/// Pinta la textura en todo el espacio disponible y captura zoom/pan.
///
/// # Arguments
/// * `ui` - UI de egui donde se dibuja el canvas.
/// * `texture` - Textura de la imagen a mostrar.
/// * `transform` - Transformación de vista (se muta con zoom/pan).
pub fn show(ui: &mut egui::Ui, texture: &egui::TextureHandle, transform: &mut ViewTransform) -> ViewResponse {
    let size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

    transform.set_viewport(Vec2::new(rect.width(), rect.height()));

    // Fondo oscuro del canvas.
    ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(24));

    // Rectángulo de la imagen en pantalla.
    let origin = transform.image_origin_screen();
    let w = transform.image_size.x * transform.zoom;
    let h = transform.image_size.y * transform.zoom;
    let image_rect = egui::Rect::from_min_size(egui::pos2(origin.x, origin.y), egui::vec2(w, h));

    ui.painter().image(
        texture.id(),
        image_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );

    let mut result = ViewResponse::default();

    // Zoom con la rueda, anclado al cursor.
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let anchor = response.hover_pos().unwrap_or_else(|| rect.center());
            let factor = (scroll * 0.001).exp();
            transform.apply_zoom_at(Vec2::new(anchor.x, anchor.y), factor);
            result.zoomed = true;
            ui.ctx().request_repaint();
        }
    }

    // Pan con arrastre.
    if response.dragged() {
        let delta = response.drag_delta();
        transform.pan_by(Vec2::new(delta.x, delta.y));
        result.panned = true;
        ui.ctx().request_repaint();
    }

    result
}
```

- [ ] **Step 2: Verificar compilación y lints**

Run: `cargo check`
Run: `cargo clippy --all-targets -- -D warnings`
Run: `cargo fmt --check`
Expected: todo pasa sin warnings. Si el API de egui 0.35 difiere (`smooth_scroll_delta`, `allocate_exact_size`, `painter().image(...)`), ajusta a la firma real del crate y verifica de nuevo.

- [ ] **Step 3: Commit**

```bash
git add src/ui/viewer.rs
git commit -m "feat: add viewer painter with wheel zoom and drag pan"
```

---

## Task 6: `src/app.rs` — Glue completo

**Files:**
- Rewrite: `src/app.rs`

- [ ] **Step 1: Escribir el glue**

Reemplazar `src/app.rs`:

```rust
//! Estado global de la aplicación y loop principal de `egui`.

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui;
use image::DynamicImage;

use crate::config::settings::Settings;
use crate::core::image_loader::load_image;
use crate::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{theme, toast::Toasts, viewer};
use crate::utils::errors::{Result, ShImagesError};
use crate::utils::paths::settings_path;

/// Evento enviado por el thread worker al UI thread.
struct LoadEvent {
    path: PathBuf,
    result: Result<DynamicImage>,
}

/// Estado global de la aplicación, creado una vez al arrancar.
///
/// `eframe` invoca [`eframe::App::ui`] en cada frame.
pub struct ShImagesApp {
    settings: Settings,
    navigation: Option<Navigation>,
    transform: ViewTransform,
    texture: Option<egui::TextureHandle>,
    rx: Option<mpsc::Receiver<LoadEvent>>,
    toasts: Toasts,
    /// `true` si el usuario ha hecho zoom/pan con la imagen actual.
    user_interacted: bool,
}

impl ShImagesApp {
    /// Crea el estado de la app cargando la configuración del usuario.
    ///
    /// Si la configuración no puede cargarse, se usan los defaults y se loguea
    /// un warning; la app nunca aborta el arranque por esto.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let settings = match settings_path().and_then(|path| Settings::load(&path)) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(error = %e, "failed to load settings; using defaults");
                Settings::default()
            }
        };
        Self {
            settings,
            navigation: None,
            transform: ViewTransform::new(Vec2::ZERO, Vec2::ZERO),
            texture: None,
            rx: None,
            toasts: Toasts::new(),
            user_interacted: false,
        }
    }

    /// Carga la configuración y devuelve un error tipado si falla.
    ///
    /// Expuesta para que los tests de integración puedan verificar el ciclo
    /// de vida completo sin arrancar una ventana.
    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }

    /// Abre el diálogo nativo y, si hay elección, carga la imagen.
    fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Imágenes", SUPPORTED_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            self.open_path(path);
        }
    }

    /// Abre `path`: construye la navegación de su carpeta y dispara la carga.
    fn open_path(&mut self, path: PathBuf) {
        match Navigation::from_folder(&path, SUPPORTED_EXTENSIONS) {
            Ok(nav) => {
                tracing::info!(path = %path.display(), "opening image");
                self.navigation = Some(nav);
                self.start_load(path);
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "failed to scan folder");
                self.toasts.push(format!("No se pudo leer la carpeta: {e}"), now());
            }
        }
    }

    /// Dispara un thread worker que carga `path` y envía el resultado por canal.
    fn start_load(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = self.ctx();
        std::thread::spawn(move || {
            let result = load_image(&path);
            let event = LoadEvent { path, result };
            if tx.send(event).is_err() {
                tracing::debug!("load event dropped (receiver gone)");
            }
            ctx.request_repaint();
        });
    }

    /// Cada frame, recoge el resultado del worker si está listo.
    fn poll_loader(&mut self, ui: &mut egui::Ui) {
        let Some(rx) = &self.rx else { return };
        let Ok(event) = rx.try_recv() else { return };
        self.rx = None;

        // Descarta resultados de navegaciones obsoletas.
        let is_current = self
            .navigation
            .as_ref()
            .and_then(|n| n.current_path())
            .map(|p| p == &event.path)
            .unwrap_or(false);
        if !is_current {
            tracing::debug!(path = %event.path.display(), "ignoring stale load result");
            return;
        }

        match event.result {
            Ok(image) => {
                tracing::info!(path = %event.path.display(), "image decoded");
                let size = image.dimensions();
                self.texture = Some(make_texture(ui.ctx(), &image));
                self.transform =
                    ViewTransform::new(Vec2::new(size.0 as f32, size.1 as f32), Vec2::ZERO);
                self.user_interacted = false;
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %event.path.display(), "failed to load image");
                self.toasts.push(format!("No se pudo abrir: {e}"), now());
            }
        }
    }

    /// Navega `dir` pasos (-1 prev, +1 next) y carga la nueva imagen.
    fn navigate(&mut self, dir: isize) {
        let Some(nav) = &mut self.navigation else { return };
        if dir > 0 {
            nav.next();
        } else {
            nav.prev();
        }
        if let Some(path) = nav.current_path().cloned() {
            self.start_load(path);
        }
    }

    /// Contexto de egui clonado para `request_repaint` desde el worker.
    fn ctx(&self) -> egui::Context {
        self.settings_pub().0.clone()
    }
}

/// Convierte una imagen decodificada en textura de egui.
fn make_texture(ctx: &egui::Context, image: &DynamicImage) -> egui::TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture("image", color_image, egui::TextureOptions::LINEAR)
}
```

> **Problema de diseño detectado:** el fragmento anterior tiene una dependencia circular: `start_load` necesita un `egui::Context` para `request_repaint` desde el worker, pero el Context se obtiene en `App::ui` (no está en `ShImagesApp`). El enfoque correcto es guardar una copia del Context al crear la app. Reescribe el archivo completo con la versión que guarda `ctx`:

```rust
//! Estado global de la aplicación y loop principal de `egui`.

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui;
use image::DynamicImage;

use crate::config::settings::Settings;
use crate::core::image_loader::load_image;
use crate::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{theme, toast::Toasts, viewer};
use crate::utils::errors::{Result, ShImagesError};
use crate::utils::paths::settings_path;

/// Evento enviado por el thread worker al UI thread.
struct LoadEvent {
    path: PathBuf,
    result: Result<DynamicImage>,
}

/// Estado global de la aplicación, creado una vez al arrancar.
///
/// `eframe` invoca [`eframe::App::ui`] en cada frame.
pub struct ShImagesApp {
    settings: Settings,
    /// Contexto de egui, clonado para `request_repaint` desde workers.
    ctx: egui::Context,
    navigation: Option<Navigation>,
    transform: ViewTransform,
    texture: Option<egui::TextureHandle>,
    rx: Option<mpsc::Receiver<LoadEvent>>,
    toasts: Toasts,
    /// `true` si el usuario ha hecho zoom/pan con la imagen actual.
    user_interacted: bool,
}

impl ShImagesApp {
    /// Crea el estado de la app cargando la configuración del usuario.
    ///
    /// Si la configuración no puede cargarse, se usan los defaults y se loguea
    /// un warning; la app nunca aborta el arranque por esto.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = match settings_path().and_then(|path| Settings::load(&path)) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(error = %e, "failed to load settings; using defaults");
                Settings::default()
            }
        };
        Self {
            settings,
            ctx: cc.egui_ctx.clone(),
            navigation: None,
            transform: ViewTransform::new(Vec2::ZERO, Vec2::ZERO),
            texture: None,
            rx: None,
            toasts: Toasts::new(),
            user_interacted: false,
        }
    }

    /// Carga la configuración y devuelve un error tipado si falla.
    ///
    /// Expuesta para que los tests de integración puedan verificar el ciclo
    /// de vida completo sin arrancar una ventana.
    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }

    /// Abre el diálogo nativo y, si hay elección, carga la imagen.
    fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Imágenes", SUPPORTED_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            self.open_path(path);
        }
    }

    /// Abre `path`: construye la navegación de su carpeta y dispara la carga.
    fn open_path(&mut self, path: PathBuf) {
        match Navigation::from_folder(&path, SUPPORTED_EXTENSIONS) {
            Ok(nav) => {
                tracing::info!(path = %path.display(), "opening image");
                self.navigation = Some(nav);
                self.start_load(path);
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "failed to scan folder");
                self.toasts.push(format!("No se pudo leer la carpeta: {e}"), now());
            }
        }
    }

    /// Dispara un thread worker que carga `path` y envía el resultado por canal.
    fn start_load(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = load_image(&path);
            let event = LoadEvent { path, result };
            if tx.send(event).is_err() {
                tracing::debug!("load event dropped (receiver gone)");
            }
            ctx.request_repaint();
        });
    }

    /// Cada frame, recoge el resultado del worker si está listo.
    fn poll_loader(&mut self, ui: &mut egui::Ui) {
        let Some(rx) = &self.rx else { return };
        let Ok(event) = rx.try_recv() else { return };
        self.rx = None;

        // Descarta resultados de navegaciones obsoletas.
        let is_current = self
            .navigation
            .as_ref()
            .and_then(|n| n.current_path())
            .map(|p| p == &event.path)
            .unwrap_or(false);
        if !is_current {
            tracing::debug!(path = %event.path.display(), "ignoring stale load result");
            return;
        }

        match event.result {
            Ok(image) => {
                tracing::info!(path = %event.path.display(), "image decoded");
                let size = image.dimensions();
                self.texture = Some(make_texture(&self.ctx, &image));
                self.transform =
                    ViewTransform::new(Vec2::new(size.0 as f32, size.1 as f32), Vec2::ZERO);
                self.user_interacted = false;
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %event.path.display(), "failed to load image");
                self.toasts.push(format!("No se pudo abrir: {e}"), now());
            }
        }
    }

    /// Navega `dir` pasos (-1 prev, +1 next) y carga la nueva imagen.
    fn navigate(&mut self, dir: isize) {
        let Some(nav) = &mut self.navigation else { return };
        if dir > 0 {
            nav.next();
        } else {
            nav.prev();
        }
        if let Some(path) = nav.current_path().cloned() {
            self.start_load(path);
        }
    }
}

/// Convierte una imagen decodificada en textura de egui.
fn make_texture(ctx: &egui::Context, image: &DynamicImage) -> egui::TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture("image", color_image, egui::TextureOptions::LINEAR)
}

/// Segundos actuales según el reloj de egui (para toasts).
fn now() -> f64 {
    // Sin contexto disponible en funciones libres: se mueve el tiempo a un
    // parámetro en las llamadas de `App::ui`. Ver Step 3.
    f64::NAN
}

impl eframe::App for ShImagesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), &self.settings.theme);

        self.poll_loader(ui);

        egui::CentralPanel::default().show(ui, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Archivo", |ui| {
                    if ui.button("Abrir…").clicked() {
                        ui.close_menu();
                        self.open_dialog();
                    }
                });
            });

            match &self.texture {
                Some(texture) => {
                    let resp = viewer::show(ui, texture, &mut self.transform);
                    if resp.zoomed || resp.panned {
                        self.user_interacted = true;
                    }
                }
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.heading("Sh_Images");
                        ui.label("Archivo → Abrir… o Ctrl+O");
                    });
                }
            }
        });

        let t = ui.input(|i| i.time);
        self.toasts.update(t);
        self.toasts.show(ui);

        self.handle_shortcuts(ui);
    }
}
```

- [ ] **Step 2: Completar el glue — eliminar `now()` libre y pasar el tiempo**

El helper `now()` libre no puede leer el reloj de egui. Sustitúyelo por un método que reciba `t`: añade dentro de `impl ShImagesApp`:

```rust
    /// Segundos actuales según el reloj de egui (para toasts).
    fn toast_now(&self, t: f64) -> f64 {
        t
    }
```

Y reemplaza todas las llamadas `now()` por `self.toast_now(t)` pasando `t` donde esté disponible. Para `open_path` y `open_dialog` (que no tienen `t`), añade un campo `now: f64` no, mejor: guarda el tiempo como parámetro. La forma limpia: `open_dialog(&mut self, t: f64)` y `open_path(&mut self, path: PathBuf, t: f64)`, y en `App::ui` se obtiene `let t = ui.input(|i| i.time);` ANTES del menú y se pasa:

```rust
fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    theme::apply(ui.ctx(), &self.settings.theme);
    let t = ui.input(|i| i.time);

    self.poll_loader(ui);

    let mut want_open = false;
    egui::CentralPanel::default().show(ui, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Archivo", |ui| {
                if ui.button("Abrir…").clicked() {
                    ui.close_menu();
                    want_open = true;
                }
            });
        });
        if want_open {
            self.open_dialog(t);
        }

        match &self.texture {
            Some(texture) => {
                let resp = viewer::show(ui, texture, &mut self.transform);
                if resp.zoomed || resp.panned {
                    self.user_interacted = true;
                }
            }
            None => {
                ui.centered_and_justified(|ui| {
                    ui.heading("Sh_Images");
                    ui.label("Archivo → Abrir… o Ctrl+O");
                });
            }
        }
    });

    self.toasts.update(t);
    self.toasts.show(ui);

    self.handle_shortcuts(ui, t);
}
```

Ajusta `open_dialog` y `open_path` a:

```rust
    /// Abre el diálogo nativo y, si hay elección, carga la imagen.
    fn open_dialog(&mut self, t: f64) {
        let picked = rfd::FileDialog::new()
            .add_filter("Imágenes", SUPPORTED_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            self.open_path(path, t);
        }
    }

    /// Abre `path`: construye la navegación de su carpeta y dispara la carga.
    fn open_path(&mut self, path: PathBuf, t: f64) {
        match Navigation::from_folder(&path, SUPPORTED_EXTENSIONS) {
            Ok(nav) => {
                tracing::info!(path = %path.display(), "opening image");
                self.navigation = Some(nav);
                self.start_load(path);
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "failed to scan folder");
                self.toasts.push(format!("No se pudo leer la carpeta: {e}"), t);
            }
        }
    }
```

En `poll_loader`, el error también necesita `t`: cambia la firma a `fn poll_loader(&mut self, ui: &mut egui::Ui, t: f64)` y usa `t` en el `push`. En el `Err` de `App::ui`, elimina la llamada a `now()` (ya no existe).

Añade `handle_shortcuts`:

```rust
    /// Atajos de teclado: Ctrl+O abre, ←→ navega, F re-ajusta a fit.
    fn handle_shortcuts(&mut self, ui: &mut egui::Ui, t: f64) {
        let open = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O));
        if open {
            self.open_dialog(t);
        }
        let next = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight));
        if next {
            self.navigate(1);
        }
        let prev = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft));
        if prev {
            self.navigate(-1);
        }
        let fit = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F));
        if fit && self.texture.is_some() {
            self.transform.fit();
            self.user_interacted = false;
        }
    }
```

> El método `ui.input_mut(|i| i.consume_key(...))` está disponible en egui 0.35 (`Ui::input_mut`). Verifícalo en compile.

- [ ] **Step 3: Eliminar el helper `now()` y `toast_now`**

El archivo final NO debe contener ni `fn now()` ni `fn toast_now`. Borra ambas y el `f64::NAN`. El resultado completo final de `src/app.rs` (sin duplicados) es el ensamblado de los Steps 1-2.

- [ ] **Step 4: Verificar compilación y lints**

Run: `cargo check`
Run: `cargo clippy --all-targets -- -D warnings`
Run: `cargo fmt --check`
Run: `cargo test`
Expected: 0 warnings, todos los tests pasan (7 view + 9 navigation + 3 toast + 6 image_loader + 3 errors + 7 settings + 5 paths = 40).

> Si el menú no cabe dentro del `Ui` raíz en eframe 0.35 o `menu::bar` difiere, usa `egui::TopBottomPanel::top("menu").show_inside(ui, |ui| { ... })` con `egui::menu::bar(ui, ...)` dentro. Verifica contra el API real de `egui-0.35.0`.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire file dialog, async loading, navigation and viewer into app"
```

---

## Task 7: `Cargo.toml` — añadir `rfd`

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Añadir la dependencia**

En `[dependencies]`, tras `egui = "0.35"`:

```toml
# Diálogo de archivos nativo (abrir imágenes) — justificación: estándar en el
# ecosistema egui, nativo por plataforma, mantenido, licencia MIT/Apache-2.0
# (AGENTS.md §7.2).
rfd = "0.15"
```

> Nota: `rfd` por defecto usa GTK3 en Linux; en Windows/macOS usa el diálogo nativo del sistema. Para CI en ubuntu, ver Task 8.

- [ ] **Step 2: Verificar compilación**

Run: `cargo check`
Expected: compila (descarga rfd). Si la última versión estable es > 0.15, ajusta.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: add rfd for native file dialogs"
```

---

## Task 8: CI — dependencias de sistema para `rfd` en Linux

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Añadir dependencias de sistema**

Añadir tras el paso `uses: Swatinem/rust-cache@v2`:

```yaml
      - name: Install Linux deps (rfd/GTK)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libgtk-3-dev
```

- [ ] **Step 2: Verificar sintaxis**

Revisión visual: el `if:` con `runner.os` es válido en GitHub Actions.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: install GTK dev libs for rfd on Linux runner"
```

---

## Task 9: Verificación final según AGENTS.md

**Files:** ninguno (QA)

- [ ] **Step 1: Correr la suite completa**

Run:
```bash
cargo check
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
cargo test --release
```

Expected: todo pasa, 0 warnings, 0 diffs de formato, ~40 tests.

- [ ] **Step 2: Verificar ausencia de panics en producción**

Run: `rg -n "unwrap\(|expect\(|unreachable!|todo!|unimplemented!" src/`
Expected: solo apariciones dentro de `#[cfg(test)]`.

- [ ] **Step 3: Verificar cobertura de `core/view.rs` y `core/navigation.rs`**

Run: `cargo test --lib`
Expected: los 7 tests de view y 9 de navigation pasan (≥80% de las funciones nuevas).

- [ ] **Step 4: Smoke test manual (solo si hay display)**

Run: `cargo run`
Expected: ventana con menú "Archivo", Ctrl+O abre el diálogo, al seleccionar una imagen se muestra con fit, rueda hace zoom en el cursor, drag hace pan, ← → navegan la carpeta, F re-fitea. Se omite en CI.

- [ ] **Step 5: Commit final (si hubo cambios de QA)**

```bash
git add -A
git commit -m "chore: final verification pass for Fase 1"
```

> Solo si la verificación modificó algo (p. ej. `cargo fmt`). Si el árbol está limpio, no hay commit.

---

## Self-Review

**Spec coverage:**
- Diálogo nativo + Ctrl+O/menú ✓ (Task 6)
- Render con TextureHandle ✓ (Task 5, 6)
- Zoom centrado en cursor ✓ (Task 1, 5)
- Pan libre ✓ (Task 1, 5)
- Fit automático + F ✓ (Task 1, 6)
- Navegación circular por carpeta filtrada ✓ (Task 2, 6)
- Carga asíncrona mínima ✓ (Task 6)
- Toasts de error ✓ (Task 4, 6)
- Fixture JPEG + multi-formato ✓ (Task 3)
- CI con deps de rfd ✓ (Task 8)
- Tests de math y navegación ✓ (Task 1, 2)

**Placeholder scan:** Sin "TBD"/"TODO". El `now()`/`toast_now`/`f64::NAN` del primer borrador se elimina explícitamente en Step 3 del Task 6 (instrucción de borrado, no código residual).

**Type consistency:**
- `ViewTransform::new(image_size, viewport)` consistente entre Task 1 y Task 6.
- `Vec2` (core) usado en `viewer::show` y `app.rs`; se convierte con `egui::vec2/pos2` en el límite UI.
- `Navigation::from_folder(path, exts)` consistente entre Task 2 y Task 6.
- `Toasts::push(msg, now)` / `update(now)` / `show(ui)` consistentes entre Task 4 y Task 6.
- `SUPPORTED_EXTENSIONS` usado en Task 2 (filtro) y Task 6 (diálogo).
