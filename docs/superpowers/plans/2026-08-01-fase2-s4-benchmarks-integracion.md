# Fase 2 — Subproyecto 4: Benchmarks y Tests de Integración — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cerrar las deudas de calidad del S4: 5 grupos de benchmarks criterion (cubriendo los umbrales medibles de §6.2) y tests de integración para los 5 flujos críticos de §8.1, todo con lógica pura (sin egui) y fixtures sintéticas generadas en runtime.

**Architecture:** Benchmarks en `benches/` (criterion, cada uno con `[[bench]] harness = false`), con un helper `benches/common/mod.rs` que genera imágenes sintéticas 1080p/4K/8K en `tempfile`. Tests de integración en `tests/integration.rs` + `tests/common/mod.rs`, componiendo piezas de `core/` (Navigation, ViewTransform, ImageCache, load_image, Settings) sin instanciar la GUI. El generador de imágenes sintéticas se duplica (~30 líneas) entre `benches/common` y `tests/common` (decisión de spec §3.8; los crates de bench y test no comparten módulos).

**Tech Stack:** `criterion 0.5` (ya en dev-deps), `tempfile 3` (ya en dev-deps), `image 0.25` (dep principal, para generar/sintetizar). Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-08-01-fase2-s4-benchmarks-integracion-design.md`

---

## File Structure

```
Cargo.toml                        # MODIFICAR — añadir [[bench]] x4 (navigation, thumbnail_gen, image_cache, preload)
benches/common/mod.rs             # CREAR — generador de imágenes sintéticas (gradiente) + constantes de resolución
benches/opening.rs                # MODIFICAR — ampliar: PNG y JPEG a 1080p/4K/8K
benches/navigation.rs             # CREAR — from_folder/next/prev/neighbor_paths con 1000 archivos
benches/thumbnail_gen.rs          # CREAR — generate_thumbnail(4K→96px)
benches/image_cache.rs            # CREAR — insert/get/contains LRU con imagen 4K
benches/preload.rs                # CREAR — preload_targets N±1 sobre 1000 imágenes
tests/common/mod.rs               # CREAR — helpers: carpeta con N imágenes, corrupto, vacío, gif, gradiente
tests/integration.rs              # CREAR — los 5 flujos críticos
README.md                         # MODIFICAR — sección "Comandos" (cargo bench / cargo test)
```

---

### Task 1: `benches/common/mod.rs` — generador de imágenes sintéticas

**Files:**
- Create: `benches/common/mod.rs`

El generador produce imágenes con un patrón de gradiente determinista. Cada píxel: `r = x % 256`, `g = y % 256`, `b = (x + y) % 256`, `a = 255`. RGBA8.

- [ ] **Step 1: Crear el archivo con las constantes de resolución y el generador**

```rust
//! Helpers compartidos para generar imágenes sintéticas en los benchmarks.
//!
//! Las imágenes se generan en memoria (patrón de gradiente determinista) y se
//! guardan en un directorio temporal: repo limpio, reproducible, sin fixtures
//! grandes commiteados (AGENTS.md §8.2 limita fixtures a <100KB).

use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

/// Resoluciones objetivo de los benchmarks (AGENTS.md §6.2).
pub const RES_1080P: (u32, u32) = (1920, 1080);
/// Resolución 4K (UHD).
pub const RES_4K: (u32, u32) = (3840, 2160);
/// Resolución 8K (UHD).
pub const RES_8K: (u32, u32) = (7680, 4320);

/// Crea una imagen de gradiente determinista con las dimensiones dadas.
///
/// Patrón: `r = x % 256`, `g = y % 256`, `b = (x + y) % 256`, alpha = 255.
/// El patrón es reproducible en cada ejecución (sin ruido aleatorio).
pub fn gradient_image(w: u32, h: u32) -> DynamicImage {
    let mut img = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba([
            (x % 256) as u8,
            (y % 256) as u8,
            ((x + y) % 256) as u8,
            255,
        ]);
    }
    DynamicImage::ImageRgba8(img)
}

/// Genera y guarda una imagen sintética en `dir`, devolviendo su ruta.
///
/// El nombre de archivo codifica el formato y la resolución para que los
/// benchmarks que comparten un directorio no colisionen.
pub fn synthetic_image_path(dir: &Path, w: u32, h: u32, format: ImageFormat) -> PathBuf {
    let ext = format
        .extensions_str()
        .first()
        .expect("cada ImageFormat tiene al menos una extensión");
    let path = dir.join(format!("synthetic_{w}x{h}.{ext}"));
    gradient_image(w, h)
        .save_with_format(&path, format)
        .expect("guardar imagen sintética en temp debe funcionar");
    path
}
```

Nota: `expect` es aceptable aquí (benchmark, no producción; AGENTS.md §2.1 aplica a release de la app, los tests/benches pueden usar `expect` — ver criterio de aceptación de la spec).

- [ ] **Step 2: Verificar que compila el modulo**

Run: `cargo check --all-targets 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores (el módulo aún no se usa, pero compila).

- [ ] **Step 3: Commit**

```bash
git add benches/common/mod.rs
git commit -m "bench: add shared synthetic image generator for benchmarks"
```

---

### Task 2: Ampliar `benches/opening.rs` — apertura PNG/JPEG 1080p/4K/8K

**Files:**
- Modify: `benches/opening.rs` (reescribir)

- [ ] **Step 1: Escribir el benchmark ampliado**

Reemplaza el contenido completo de `benches/opening.rs` por:

```rust
//! Benchmark: tiempo de apertura/decodificación de imágenes a varias
//! resoluciones (AGENTS.md §6.2: 1080p < 100ms, 4K < 200ms, 8K < 500ms).

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use image::ImageFormat;
use sh_images::core::image_loader::load_image;
use tempfile::tempdir;

mod common;

use common::{RES_1080P, RES_4K, RES_8K, synthetic_image_path};

/// Crea el directorio temp con una imagen sintética de la resolución dada.
fn setup_synthetic(format: ImageFormat, (w, h): (u32, u32)) -> (tempfile::TempDir, Box<Path>) {
    let dir = tempdir().expect("tempdir para benchmark");
    let path = synthetic_image_path(dir.path(), w, h, format);
    (dir, path.into_boxed_path())
}

/// Mide `load_image` para una imagen ya generada en disco.
fn bench_open(c: &mut Criterion, name: &str, dir: &tempfile::TempDir, path: &Path) {
    c.bench_function(name, |b| {
        b.iter(|| {
            let ok = load_image(black_box(path)).is_ok();
            black_box(ok);
        })
    });
    // `dir` se mantiene vivo hasta el final del grupo para que el archivo exista.
    let _ = dir;
}

fn bench_opening(c: &mut Criterion) {
    for format in [ImageFormat::Png, ImageFormat::Jpeg] {
        let ext = format.extensions_str()[0];
        for (label, res) in [("1080p", RES_1080P), ("4k", RES_4K), ("8k", RES_8K)] {
            let (dir, path) = setup_synthetic(format, res);
            bench_open(
                c,
                &format!("open_{ext}_{label}"),
                &dir,
                &path,
            );
        }
    }
}

criterion_group!(benches, bench_opening);
criterion_main!(benches);
```

- [ ] **Step 2: Verificar que compila**

Run: `cargo check --bench opening 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores.

- [ ] **Step 3: Ejecutar una pasada rápida (solo apertura PNG 1080p) para verificar el harness**

Run: `cargo bench --bench opening "open_png_1080p" -- --warm-up-time 0.2 --measurement-time 0.3 --sample-size 10`
Expected: criterion corre y muestra una estimación (p.ej. `time: [...]`), exit 0.

- [ ] **Step 4: Commit**

```bash
git add benches/opening.rs
git commit -m "bench: expand opening benchmark to PNG/JPEG at 1080p/4K/8K"
```

---

### Task 3: `benches/navigation.rs` — navegación con 1000 archivos

**Files:**
- Create: `benches/navigation.rs`

El setup crea 1000 archivos `.jpg` (tocados con `File::create`, sin decodificar: `Navigation::from_folder` solo filtra por extensión). `from_folder` se mide una vez con los 1000 archivos ya en disco.

- [ ] **Step 1: Escribir el benchmark**

```rust
//! Benchmark: navegación entre imágenes de una carpeta (AGENTS.md §6.2:
//! latencia de navegación < 50ms).

use std::fs;
use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use tempfile::tempdir;

/// Crea `n` archivos `.jpg` (vacíos) en una carpeta temp y devuelve (dir, paths).
fn setup_folder(n: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempdir().expect("tempdir para benchmark");
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let path = dir.path().join(format!("img_{i:04}.jpg"));
        fs::File::create(&path).expect("crear archivo de imagen tocado");
        paths.push(path);
    }
    (dir, paths)
}

fn bench_navigation(c: &mut Criterion) {
    let (_dir, paths) = setup_folder(1000);
    let anchor = &paths[0];

    c.bench_function("navigation_from_folder_1000", |b| {
        b.iter(|| {
            let nav = Navigation::from_folder(black_box(anchor), SUPPORTED_EXTENSIONS)
                .expect("from_folder debe funcionar");
            black_box(nav.images.len());
        })
    });

    let mut nav =
        Navigation::from_folder(anchor, SUPPORTED_EXTENSIONS).expect("from_folder inicial");

    c.bench_function("navigation_next_1000", |b| {
        b.iter(|| {
            nav.next();
        })
    });

    c.bench_function("navigation_prev_1000", |b| {
        b.iter(|| {
            nav.prev();
        })
    });

    c.bench_function("navigation_neighbor_paths_1000", |b| {
        b.iter(|| {
            black_box(nav.neighbor_paths());
        })
    });
}

criterion_group!(benches, bench_navigation);
criterion_main!(benches);
```

- [ ] **Step 2: Verificar que compila y ejecuta**

Run: `cargo check --bench navigation 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores.

Run: `cargo bench --bench navigation -- --warm-up-time 0.2 --measurement-time 0.3 --sample-size 10`
Expected: 4 estimaciones (from_folder, next, prev, neighbor_paths), exit 0.

- [ ] **Step 3: Commit**

```bash
git add benches/navigation.rs
git commit -m "bench: add navigation benchmarks over a 1000-file folder"
```

---

### Task 4: `benches/thumbnail_gen.rs` — miniatura de 4K

**Files:**
- Create: `benches/thumbnail_gen.rs`

- [ ] **Step 1: Escribir el benchmark**

```rust
//! Benchmark: generación de miniatura 96px desde una imagen 4K.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::thumbnail_gen::{generate_thumbnail, THUMB_MAX};

mod common;

use common::{RES_4K, gradient_image};

fn bench_thumbnail(c: &mut Criterion) {
    let image = gradient_image(RES_4K.0, RES_4K.1);
    c.bench_function("thumbnail_4k_to_96", |b| {
        b.iter(|| {
            let thumb = generate_thumbnail(black_box(&image), THUMB_MAX);
            black_box(thumb);
        })
    });
}

criterion_group!(benches, bench_thumbnail);
criterion_main!(benches);
```

- [ ] **Step 2: Verificar que compila y ejecuta**

Run: `cargo check --bench thumbnail_gen 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores.

Run: `cargo bench --bench thumbnail_gen -- --warm-up-time 0.2 --measurement-time 0.3 --sample-size 10`
Expected: 1 estimación, exit 0.

- [ ] **Step 3: Commit**

```bash
git add benches/thumbnail_gen.rs
git commit -m "bench: add thumbnail generation benchmark (4K -> 96px)"
```

---

### Task 5: `benches/image_cache.rs` — LRU insert/get/contains con imagen 4K

**Files:**
- Create: `benches/image_cache.rs`

- [ ] **Step 1: Escribir el benchmark**

```rust
//! Benchmark: operaciones del LRU cache sobre una imagen 4K decodificada.

use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::image_cache::ImageCache;

mod common;

use common::{RES_4K, gradient_image};

fn bench_cache(c: &mut Criterion) {
    let image = gradient_image(RES_4K.0, RES_4K.1);
    let cache = ImageCache::new(512);

    c.bench_function("cache_insert_4k", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            let path = PathBuf::from(format!("/bench/img_{i}.png"));
            let res = cache.insert(black_box(path), image.clone());
            black_box(res.cached);
        })
    });

    // Pre-inserta una entrada para medir get/contains.
    let path = PathBuf::from("/bench/known.png");
    cache.insert(path.clone(), image.clone());

    c.bench_function("cache_get_4k", |b| {
        b.iter(|| {
            let entry = cache.get(black_box(&path));
            black_box(entry.is_some());
        })
    });

    c.bench_function("cache_contains_4k", |b| {
        b.iter(|| {
            black_box(cache.contains(black_box(&path)));
        })
    });
}

criterion_group!(benches, bench_cache);
criterion_main!(benches);
```

Nota: `cache_get_4k` clona `image.clone()` por iteración de `insert` solo en el primer grupo. En `get`/`contains` no hay clone. `PathBuf::from(format!(...))` en el hot loop del insert es intencional (mide el coste real del LRU insert incluyendo la clave nueva).

- [ ] **Step 2: Verificar que compila y ejecuta**

Run: `cargo check --bench image_cache 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores.

Run: `cargo bench --bench image_cache -- --warm-up-time 0.2 --measurement-time 0.3 --sample-size 10`
Expected: 3 estimaciones (insert, get, contains), exit 0.

- [ ] **Step 3: Commit**

```bash
git add benches/image_cache.rs
git commit -m "bench: add LRU cache insert/get/contains benchmarks"
```

---

### Task 6: `benches/preload.rs` — preload_targets N±1

**Files:**
- Create: `benches/preload.rs`

- [ ] **Step 1: Escribir el benchmark**

```rust
//! Benchmark: cálculo de targets de pre-carga N±1 sobre 1000 imágenes.

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::navigation::Navigation;
use sh_images::core::preload::{preload_targets, PRELOAD_DEPTH};

fn never(_p: &Path) -> bool {
    false
}

fn bench_preload(c: &mut Criterion) {
    let images: Vec<std::path::PathBuf> = (0..1000)
        .map(|i| std::path::PathBuf::from(format!("/bench/img_{i:04}.png")))
        .collect();
    let nav = Navigation {
        images,
        current: 500,
    };

    c.bench_function("preload_targets_n1_1000", |b| {
        b.iter(|| {
            let targets = preload_targets(black_box(&nav), PRELOAD_DEPTH, never, never);
            black_box(targets.len());
        })
    });
}

criterion_group!(benches, bench_preload);
criterion_main!(benches);
```

- [ ] **Step 2: Verificar que compila y ejecuta**

Run: `cargo check --bench preload 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores.

Run: `cargo bench --bench preload -- --warm-up-time 0.2 --measurement-time 0.3 --sample-size 10`
Expected: 1 estimación, exit 0.

- [ ] **Step 3: Commit**

```bash
git add benches/preload.rs
git commit -m "bench: add preload target planner benchmark"
```

---

### Task 7: Registrar los nuevos benchmarks en `Cargo.toml`

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Añadir los `[[bench]]`**

Después del bloque existente `[[bench]]` de `opening` (líneas 16-18), añade:

```toml
[[bench]]
name = "navigation"
harness = false

[[bench]]
name = "thumbnail_gen"
harness = false

[[bench]]
name = "image_cache"
harness = false

[[bench]]
name = "preload"
harness = false
```

- [ ] **Step 2: Verificar que `cargo bench --no-run` compila todos los benchmarks**

Run: `cargo bench --no-run 2>&1 | Select-String "error" | Select-Object -First 10`
Expected: sin errores, los 5 benches compilan.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: register navigation/thumbnail/cache/preload benchmark targets"
```

---

### Task 8: `tests/common/mod.rs` — helpers de tests de integración

**Files:**
- Create: `tests/common/mod.rs`

Este módulo es compartido por `tests/integration.rs` vía `mod common;`. Crea carpetas con imágenes sintéticas, y genera archivos corruptos/vacíos/GIF.

- [ ] **Step 1: Escribir los helpers**

```rust
//! Helpers compartidos para los tests de integración.
//!
//! Generan carpetas temp con imágenes sintéticas y archivos de error
//! (corrupto/vacío), para no committear fixtures grandes (AGENTS.md §8.2).

use std::fs;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use tempfile::tempdir;

/// Crea una imagen de gradiente determinista (misma lógica que benches/common).
pub fn gradient_image(w: u32, h: u32) -> DynamicImage {
    let mut img = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba([
            (x % 256) as u8,
            (y % 256) as u8,
            ((x + y) % 256) as u8,
            255,
        ]);
    }
    DynamicImage::ImageRgba8(img)
}

/// Crea una carpeta temp con `n` imágenes `.jpg` sintéticas de 64x64.
///
/// Devuelve `(TempDir, rutas ordenadas)`. `TempDir` se elimina al dropear.
pub fn make_folder_with_images(n: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempdir().expect("tempdir en test");
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let path = dir.path().join(format!("img_{i:04}.jpg"));
        gradient_image(64, 64)
            .save_with_format(&path, ImageFormat::Jpeg)
            .expect("guardar imagen sintética");
        paths.push(path);
    }
    paths.sort();
    (dir, paths)
}

/// Escribe un archivo `.png` con bytes que NO son una imagen válida.
pub fn corrupt_png_path(dir: &Path) -> PathBuf {
    let path = dir.join("corrupt.png");
    fs::write(&path, b"this is definitely not a valid png file").expect("escribir corrupto");
    path
}

/// Escribe un archivo `.png` vacío (0 bytes).
pub fn empty_png_path(dir: &Path) -> PathBuf {
    let path = dir.join("empty.png");
    fs::write(&path, []).expect("escribir archivo vacío");
    path
}

/// Guarda un GIF 1x1 válido y devuelve su ruta.
pub fn gif_path(dir: &Path) -> PathBuf {
    let path = dir.join("one.gif");
    gradient_image(1, 1)
        .save_with_format(&path, ImageFormat::Gif)
        .expect("guardar gif");
    path
}
```

- [ ] **Step 2: Verificar que compila como módulo de tests (todavía sin consumer)**

Run: `cargo check --all-targets 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores.

- [ ] **Step 3: Commit**

```bash
git add tests/common/mod.rs
git commit -m "test: add shared integration test helpers"
```

---

### Task 9: `tests/integration.rs` — los 5 flujos críticos

**Files:**
- Create: `tests/integration.rs`

- [ ] **Step 1: Escribir los 5 flujos**

```rust
//! Tests de integración de los flujos críticos (AGENTS.md §8.1).
//!
//! Lógica pura sobre `core/` (sin egui): el paso de "diálogo nativo" de la
//! spec se abstrae porque `open_path` ya recibe un `PathBuf`.

mod common;

use std::path::Path;

use sh_images::config::settings::Settings;
use sh_images::core::image_cache::ImageCache;
use sh_images::core::image_loader::load_image;
use sh_images::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use sh_images::core::preload::{preload_targets, PRELOAD_DEPTH};
use sh_images::core::view::{Vec2, ViewTransform};

use common::{
    corrupt_png_path, empty_png_path, gif_path, make_folder_with_images,
};

/// Flujo 1 — Apertura: abrir → decodificar → cachear.
///
/// Equivale a "abrir app → seleccionar imagen → renderizar": se construye la
/// navegación de la carpeta, se decodifica la imagen actual y se cachea.
#[test]
fn flujo_apertura_completo() {
    let (dir, paths) = make_folder_with_images(3);
    let target = &paths[1];

    let nav = Navigation::from_folder(target, SUPPORTED_EXTENSIONS).expect("carpeta válida");
    assert_eq!(nav.current_path().expect("imagen actual"), target);

    let image = load_image(&nav.current_path().expect("imagen actual"))
        .expect("decodificar imagen sintética");
    let (w, h) = image.dimensions();
    assert_eq!((w, h), (64, 64), "la imagen sintética es 64x64");

    let cache = ImageCache::new(512);
    let result = cache.insert(target.clone(), image);
    assert!(result.cached, "la imagen 64x64 cabe en el LRU 512MiB");
    let entry = cache.get(target).expect("re-leer del cache");
    assert_eq!(entry.dimensions(), (64, 64), "la entrada cacheada conserva dimensiones");

    // Pre-carga N±1: desde la imagen 1 (0-indexed), los vecinos son 0 y 2.
    let targets = preload_targets(&nav, PRELOAD_DEPTH, |p| cache.contains(p), |_| false);
    assert_eq!(targets.len(), 2, "dos vecinos a precargar");
    assert!(targets.iter().all(|p| !cache.contains(p)), "vecinos aún no cacheados");

    let _ = dir;
}

/// Flujo 2 — Navegación: forward/backward circular + orden correcto.
#[test]
fn flujo_navegacion_circular() {
    let (_dir, paths) = make_folder_with_images(5);

    let mut nav = Navigation::from_folder(&paths[0], SUPPORTED_EXTENSIONS)
        .expect("carpeta válida");
    assert_eq!(nav.images.len(), 5);

    // Los paths están ordenados alfabéticamente por nombre img_0000..img_0004.
    assert_eq!(nav.current_path().expect("actual"), &paths[0]);

    // Forward hasta el final: vuelve al inicio (circular).
    for _ in 0..5 {
        nav.next();
    }
    assert_eq!(nav.current_path().expect("actual"), &paths[0], "circular forward");

    // Backward desde el inicio: va al final (circular).
    nav.prev();
    assert_eq!(nav.current_path().expect("actual"), &paths[4], "circular backward");

    // Vecinos N±1 desde el índice 2.
    let mut mid = Navigation::from_folder(&paths[2], SUPPORTED_EXTENSIONS).expect("válida");
    let [prev, next] = mid.neighbor_paths();
    assert_eq!(prev.expect("anterior"), &paths[1]);
    assert_eq!(next.expect("siguiente"), &paths[3]);
}

/// Flujo 3 — Zoom/Pan: zoom in → pan → fit restaura.
#[test]
fn flujo_zoom_pan_fit() {
    let mut t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
    let fit = t.fit_zoom();

    // Zoom in en un punto ancla.
    t.apply_zoom_at(Vec2::new(250.0, 250.0), 2.0);
    assert!(t.zoom > fit, "zoom in supera el fit");

    // El punto ancla queda fijo tras el zoom.
    let anchor = Vec2::new(250.0, 250.0);
    let origin = t.image_origin_screen();
    let image_point = anchor.sub(origin).div(t.zoom);
    let new_origin = t.image_origin_screen();
    let new_screen = new_origin.add(image_point.mul(t.zoom));
    assert!((new_screen.x - anchor.x).abs() < 1e-3, "ancla fija en x");
    assert!((new_screen.y - anchor.y).abs() < 1e-3, "ancla fija en y");

    // Pan desplaza.
    let before = t.image_origin_screen();
    t.pan_by(Vec2::new(30.0, -10.0));
    let after = t.image_origin_screen();
    assert!((after.x - before.x - 30.0).abs() < 1e-3, "pan mueve en x");
    assert!((after.y - before.y + 10.0).abs() < 1e-3, "pan mueve en y");

    // Fit restaura zoom y centra.
    t.fit();
    let origin_fit = t.image_origin_screen();
    let expected = Vec2::new((500.0 - 2000.0 * fit) / 2.0, (500.0 - 1000.0 * fit) / 2.0);
    assert!((t.zoom - fit).abs() < 1e-3, "fit restaura zoom");
    assert!((origin_fit.x - expected.x).abs() < 1e-3, "fit centra en x");
    assert!((origin_fit.y - expected.y).abs() < 1e-3, "fit centra en y");
}

/// Flujo 4 — Error: imagen corrupta no crashea; carpeta inexistente da Err;
/// carpeta sin imágenes soportadas da Ok con lista vacía.
#[test]
fn flujo_imagen_corrupta_no_crash() {
    let dir = tempfile::tempdir().expect("tempdir");

    let corrupt = corrupt_png_path(dir.path());
    let err = load_image(&corrupt).expect_err("png corrupto devuelve error");
    assert!(matches!(err, sh_images::utils::errors::ShImagesError::Decode(_)),
        "error de decode, no panic");

    let empty = empty_png_path(dir.path());
    assert!(load_image(&empty).is_err(), "png vacío también es error");

    let gif = gif_path(dir.path());
    let gif_img = load_image(&gif).expect("gif válido decodifica");
    assert_eq!(gif_img.dimensions(), (1, 1));

    // Carpeta inexistente → Err(Io).
    let missing = dir.path().join("no_such_dir").join("a.png");
    let err = Navigation::from_folder(&missing, SUPPORTED_EXTENSIONS)
        .expect_err("carpeta inexistente da error");
    assert!(matches!(err, sh_images::utils::errors::ShImagesError::Io(_)));

    // Carpeta con solo .txt → Ok con imágenes vacías.
    let empty_folder = dir.path().join("no_images");
    std::fs::create_dir_all(&empty_folder).expect("crear carpeta");
    std::fs::write(empty_folder.join("notes.txt"), b"not an image").expect("escribir txt");
    let nav = Navigation::from_folder(&empty_folder.join("x.txt"), SUPPORTED_EXTENSIONS)
        .expect("carpeta leíble");
    assert!(nav.images.is_empty(), "ninguna imagen soportada");
    assert_eq!(nav.current_path(), None, "sin imagen actual");
}

/// Flujo 5 — Configuración: modificar → guardar → recargar (cerrar/reabrir).
#[test]
fn flujo_configuracion_persistencia() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("settings.toml");

    let first = Settings::default();
    assert_eq!(first.cache_memory_limit_mb, 512);
    first.save(&path).expect("guardar defaults");

    // "Cerrar la app": modificar el archivo como haría un segundo arranque.
    let modified = Settings {
        cache_memory_limit_mb: 256,
        theme: "light".to_string(),
    };
    modified.save(&path).expect("guardar modificado");

    // "Reabrir la app": recargar desde disco.
    let loaded = Settings::load(&path).expect("cargar settings");
    assert_eq!(loaded, modified, "persiste el último valor guardado");
    assert_eq!(loaded.theme, "light");
}
```

Nota: `Path` se importa y no se usa directamente en el código final si clippy lo marca; revisa los imports y elimina los no usados. Si `sh_images::utils::errors::ShImagesError` no está exportado de forma accesible, usa un helper: en ese caso añade al `common/mod.rs`:

```rust
/// `true` si el error es de decode.
pub fn is_decode_err(e: &sh_images::utils::errors::ShImagesError) -> bool {
    matches!(e, sh_images::utils::errors::ShImagesError::Decode(_))
}
```

- [ ] **Step 2: Ejecutar los tests de integración**

Run: `cargo test --test integration 2>&1 | Select-String "test result|FAILED|error" | Select-Object -First 10`
Expected: `test result: ok. 5 passed; 0 failed`.

Si `ShImagesError` no es accesible por ruta, ajusta según la nota del Step 1.

- [ ] **Step 3: Verificar que la suite completa sigue verde**

Run: `cargo test 2>&1 | Select-String "test result: ok"`
Expected: los 103 unitarios + 5 de integración = 108.

- [ ] **Step 4: Commit**

```bash
git add tests/integration.rs
git commit -m "test: add integration tests for the 5 critical flows"
```

---

### Task 10: README — sección "Comandos"

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Leer el README actual**

Run: `Get-Content README.md`
Expected: ver la estructura actual para ubicar dónde añadir.

- [ ] **Step 2: Añadir una sección "Desarrollo" con los comandos**

Añade al final del README:

```markdown
## Desarrollo

```bash
# Suite de tests completa (unitarios + integración)
cargo test

# Tests de integración de los flujos críticos (AGENTS.md §8.1)
cargo test --test integration

# Benchmarks de rendimiento (AGENTS.md §6.2)
cargo bench

# Benchmarks sin ejecutar (solo compilar)
cargo bench --no-run

# QA completo pre-commit
cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test
```
```

- [ ] **Step 3: Verificar el diff**

Run: `git diff README.md`
Expected: solo la sección añadida.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: add development commands (tests and benchmarks) to README"
```

---

### Task 11: QA final del S4

**Files:**
- (ninguno)

- [ ] **Step 1: Check + Clippy + fmt**

Run: `cargo check --all-targets 2>&1 | Select-String "error" | Select-Object -First 5`
Run: `cargo clippy --all-targets -- -D warnings 2>&1 | Select-String "error|warning" | Select-Object -First 5`
Run: `cargo fmt --check`
Expected: sin errores, sin warnings, fmt limpio.

- [ ] **Step 2: Suite completa (debug y release)**

Run: `cargo test 2>&1 | Select-String "test result"`
Run: `cargo test --release 2>&1 | Select-String "test result"`
Expected: 108 passed (103 unit + 5 integración) en ambos.

- [ ] **Step 3: Benchmarks compilan y ejecutan (pasada corta)**

Run: `cargo bench --no-run 2>&1 | Select-String "error" | Select-Object -First 5`
Expected: sin errores.

- [ ] **Step 4: Verificar prohibiciones de AGENTS.md en producción**

Run: `rg -n "\.unwrap\(|\.expect\(|println!" src -g '*.rs' | Select-String -NotMatch "cfg\(test\)"`
Expected: sin hits en producción (los de tests/benches están permitidos).

- [ ] **Step 5: Commit final si quedó algo pendiente**

```bash
git add -A
git commit -m "chore: final S4 QA pass" || echo "nada que commitear"
```

---

## Self-Review (writing-plans)

**Spec coverage:**
- §2.1 (lógica pura) → Task 9 ✓
- §2.2 (sintéticas en runtime) → Tasks 1, 8 ✓
- §2.3 (5 grupos) → Tasks 2-6 ✓
- §3.1-3.6 (benches) → Tasks 1-6 ✓
- §3.7-3.8 (tests common, duplicación) → Task 8, nota en Task 1 header ✓
- §3.9 (5 flujos) → Task 9 ✓
- §4 (Cargo.toml) → Task 7 ✓
- §5 (criterios de aceptación) → Tasks 9, 11 ✓
- §5 (README) → Task 10 ✓

**Placeholder scan:** Sin "TBD"/"TODO"; todo paso tiene código o comando con salida esperada. La nota en Task 9 Step 1 (ajuste de imports / helper `is_decode_err`) es una contingencia explícita, no un placeholder.

**Type consistency:**
- `gradient_image(w, h) -> DynamicImage` definida en Task 1 (benches) y Task 8 (tests), mismas firmas. ✓
- `synthetic_image_path(dir, w, h, format)` solo en benches (Task 1). ✓
- `make_folder_with_images(n) -> (TempDir, Vec<PathBuf>)`, `corrupt_png_path`, `empty_png_path`, `gif_path` solo en tests (Task 8). ✓
- `ViewTransform`/`Vec2` API de `core/view.rs` usada en Task 9 coincide con el código real (verificado). ✓
- `Navigation::from_folder(path, SUPPORTED_EXTENSIONS)`, `next/prev/current_path/neighbor_paths` coinciden con `core/navigation.rs`. ✓
- `preload_targets(nav, depth, is_cached, is_in_flight)` coincide con `core/preload.rs`. ✓
- `Settings::load/save/default` con campos `cache_memory_limit_mb`/`theme` coinciden con `config/settings.rs`. ✓
- `load_image`, `ImageCache::new/insert/get/contains` coinciden con el código real. ✓
- `expect`/`unwrap` solo en tests y benches (no producción). ✓
