# Fase 4 — GIF animado + Slideshow — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Completar la Fase 4 de Sh_Images: reproducción de GIF animado (respetando retardos por frame, en bucle) y slideshow automático (intervalo configurable en settings + atajos de velocidad, pausa al interactuar).

**Architecture:** `core/image_loader.rs` introduce `LoadedImage::{Static, Animated}`; el `ImageCache` guarda `LoadedImage` (cuenta la memoria de todos los frames) con un shim `insert(DynamicImage)` para no romper callers. La app guarda `AnimState` y reconstruye la textura al cambiar de frame (`frame_index_at`/`time_to_next_frame` puros en core). `core/slideshow.rs` (puro) define intervalos; `Action::ToggleSlideshow`/`Faster`/`Slower` (F5 / "," / "."), campo `settings.slideshow_interval_secs`, y `app.rs` orquesta auto-avance + pausa por interacción.

**Tech Stack:** Rust, `image` 0.25 (`GifDecoder`, `GifEncoder` para fixtures, `ImageReader`), `egui`/`eframe`, `thiserror`, `serde`/`toml`, `insta` (dev), `tempfile` (dev).

**Spec:** `docs/superpowers/specs/2026-08-03-fase4-gif-slideshow-design.md`

---

### Task 1: `LoadedImage` en loader + cache (end-to-end, sin GIF aún)

**Files:**
- Modify: `src/core/image_loader.rs`
- Modify: `src/core/image_cache.rs`
- Modify: `src/app.rs`
- Modify: `tests/integration.rs`
- Test: `cargo test -p sh_images --lib core::image_loader core::image_cache`

En esta tarea `load_image` sigue devolviendo el primer frame (un GIF queda como `Static`); la rama de animación llega en la Task 2. El objetivo es migrar el pipeline a `LoadedImage` sin romper nada.

- [ ] **Step 1: Reescribir `src/core/image_loader.rs` (parte producción)**

Reemplaza TODO el contenido de `src/core/image_loader.rs` por:

```rust
//! Carga y decodificación síncrona de imágenes.

use std::path::Path;

use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};

use crate::utils::errors::{Result, ShImagesError};

/// Un frame de una imagen animada: buffer RGBA ya compuesto + retardo.
pub struct AnimatedFrame {
    pub image: DynamicImage,
    pub delay: std::time::Duration,
}

/// Imagen animada (GIF): frames en orden de reproducción y duración total.
///
/// El loader garantiza `frames` no vacío y `total_duration > 0`.
pub struct AnimatedImage {
    pub frames: Vec<AnimatedFrame>,
    pub total_duration: std::time::Duration,
}

/// Imagen decodificada: estática o animada.
pub enum LoadedImage {
    Static(DynamicImage),
    Animated(AnimatedImage),
}

impl From<DynamicImage> for LoadedImage {
    fn from(image: DynamicImage) -> Self {
        LoadedImage::Static(image)
    }
}

impl LoadedImage {
    /// `true` si la imagen tiene animación (varios frames).
    pub fn is_animated(&self) -> bool {
        matches!(self, LoadedImage::Animated(_))
    }

    /// Dimensiones de la imagen (las del primer frame; todos comparten tamaño).
    pub fn dimensions(&self) -> (u32, u32) {
        self.first_frame().dimensions()
    }

    /// Primer frame (imagen completa para `Static`).
    pub fn first_frame(&self) -> &DynamicImage {
        match self {
            LoadedImage::Static(img) => img,
            LoadedImage::Animated(anim) => &anim.frames[0].image,
        }
    }
}

/// Carga y decodifica una imagen desde el filesystem.
///
/// Por ahora devuelve el primer frame como `Static` (la rama GIF animado se
/// añade en la Task 2).
pub fn load_image(path: &Path) -> Result<LoadedImage> {
    let reader = ImageReader::open(path)?;
    let reader = reader.with_guessed_format()?;
    let image = reader.decode().map_err(map_image_error)?;
    Ok(LoadedImage::Static(image))
}

fn map_image_error(e: image::ImageError) -> ShImagesError {
    match e {
        image::ImageError::IoError(io) => ShImagesError::Io(io),
        image::ImageError::Unsupported(msg) => ShImagesError::UnsupportedFormat(msg.to_string()),
        other => ShImagesError::Decode(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.png")
    }

    #[test]
    fn decoding_valid_png_returns_static_image() {
        let img = load_image(&fixture()).unwrap();
        assert_eq!(img.dimensions(), (1, 1));
        assert!(!img.is_animated());
        assert_eq!(img.first_frame().width(), 1);
    }

    #[test]
    fn decoding_valid_jpeg_returns_static_image() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.jpg");
        let img = load_image(&path).unwrap();
        assert_eq!(img.dimensions(), (16, 16));
    }

    #[test]
    fn loading_missing_file_returns_io_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does_not_exist.png");
        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn truncated_png_returns_decode_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("truncated.png");
        let bytes = fs::read(fixture()).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(
            err,
            ShImagesError::Decode(_) | ShImagesError::Io(_)
        ));
    }

    #[test]
    fn garbage_content_with_png_extension_returns_decode_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("random.png");
        fs::write(&path, b"this is definitely not an image").unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::Decode(_)));
    }

    #[test]
    fn unknown_extension_returns_unsupported_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("random.xyz");
        fs::write(&path, b"this is definitely not an image").unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::UnsupportedFormat(_)));
    }
}
```

- [ ] **Step 2: Migrar `src/core/image_cache.rs` a `LoadedImage`**

Cambios puntuales (no reescribir el archivo entero):

1. Imports: añadir
```rust
use std::time::Duration;

use crate::core::image_loader::{AnimatedFrame, AnimatedImage, LoadedImage};
```

2. `struct CacheEntry` pasa a:
```rust
struct CacheEntry {
    image: LoadedImage,
    bytes: u64,
}
```

3. `CacheInner::insert` cambia la firma a `LoadedImage` y el coste se calcula con un nuevo helper:
```rust
    fn insert(&mut self, path: PathBuf, image: LoadedImage) -> InsertResult {
        let bytes = estimate_loaded_bytes(&image);
```

4. Añadir el helper de coste (tras `estimate_bytes`):
```rust
/// Coste en bytes de un `LoadedImage`: la suma de todos sus frames si es
/// animado.
fn estimate_loaded_bytes(image: &LoadedImage) -> u64 {
    match image {
        LoadedImage::Static(img) => estimate_bytes(img),
        LoadedImage::Animated(anim) => anim
            .frames
            .iter()
            .map(|f| estimate_bytes(&f.image))
            .sum(),
    }
}
```

5. Añadir `insert_loaded` y mantener `insert(DynamicImage)` como shim:
```rust
    /// Inserta una `LoadedImage` decodificada; evicta en orden LRU hasta caber.
    ///
    /// All-or-nothing: si la imagen sola excede el límite, no cachea ni evicta.
    pub fn insert_loaded(&self, path: PathBuf, image: LoadedImage) -> InsertResult {
        self.lock().insert(path, image)
    }

    /// Inserta una imagen estática decodificada (shim de `insert_loaded`).
    ///
    /// Conveniencia para callers que solo manejan `DynamicImage`.
    pub fn insert(&self, path: PathBuf, image: DynamicImage) -> InsertResult {
        self.insert_loaded(path, LoadedImage::Static(image))
    }
```

6. `CacheEntryRef` deref pasa a `&LoadedImage`:
```rust
impl Deref for CacheEntryRef<'_> {
    type Target = LoadedImage;
    fn deref(&self) -> &LoadedImage {
        &self.guard.nodes[self.index].value.image
    }
}
```

7. Tests: añadir casos nuevos y actualizar el import. Al final del `mod tests` de `image_cache.rs` añade:
```rust
    use crate::core::image_loader::{AnimatedFrame, AnimatedImage, LoadedImage};
    use std::time::Duration;

    fn animated() -> LoadedImage {
        LoadedImage::Animated(AnimatedImage {
            frames: vec![
                AnimatedFrame {
                    image: rgba(16, 16),
                    delay: Duration::from_millis(100),
                },
                AnimatedFrame {
                    image: rgba(16, 16),
                    delay: Duration::from_millis(200),
                },
            ],
            total_duration: Duration::from_millis(300),
        })
    }

    #[test]
    fn insert_animated_counts_sum_of_frames() {
        let cache = ImageCache::new(4);
        let res = cache.insert_loaded(PathBuf::from("anim.gif"), animated());
        assert!(res.cached);
        assert_eq!(cache.memory_used(), 2 * 16 * 16 * 4);
    }

    #[test]
    fn animated_entry_is_readable_and_flagged() {
        let cache = ImageCache::new(4);
        cache.insert_loaded(PathBuf::from("anim.gif"), animated());
        let entry = cache
            .get(Path::new("anim.gif"))
            .expect("debería estar cacheada");
        assert!(entry.is_animated());
        assert_eq!(entry.dimensions(), (16, 16));
    }

    #[test]
    fn animated_lru_eviction_respects_summed_size() {
        // Límite 1 MiB = caben 4 imágenes de 256 KiB. Dos imágenes animadas de
        // 2 frames (512 KiB cada una) ocupan 2 slots.
        let cache = ImageCache::new(1);
        let res = cache.insert_loaded(PathBuf::from("a.gif"), animated_big());
        assert!(res.cached);
        let res2 = cache.insert_loaded(PathBuf::from("b.gif"), animated_big());
        assert!(res2.cached);
        assert_eq!(cache.len(), 2);
        // Una tercera animada del mismo tamaño fuerza evicción de `a.gif`.
        let res3 = cache.insert_loaded(PathBuf::from("c.gif"), animated_big());
        assert!(res3.cached);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(Path::new("a.gif")).is_none());
        assert!(cache.get(Path::new("b.gif")).is_some());
        assert!(cache.get(Path::new("c.gif")).is_some());
    }

    fn animated_big() -> LoadedImage {
        // 2 frames de 256x256 RGBA = 2 * 262144 B = 512 KiB.
        LoadedImage::Animated(AnimatedImage {
            frames: vec![
                AnimatedFrame {
                    image: rgba(256, 256),
                    delay: Duration::from_millis(100),
                },
                AnimatedFrame {
                    image: rgba(256, 256),
                    delay: Duration::from_millis(100),
                },
            ],
            total_duration: Duration::from_millis(200),
        })
    }
```

> Nota: los tests existentes de `image_cache.rs` llaman a `cache.insert(path, rgba(...))`; el shim `insert(DynamicImage)` los deja intactos (no editar esas llamadas).

- [ ] **Step 3: Actualizar `src/app.rs` (worker de miniaturas + textura)**

En `new`, dentro del worker de miniaturas, cambiar el manejo del resultado de `load_image`:

```rust
                    match image {
                        Ok(image) => {
                            let thumb = generate_thumbnail(image.first_frame(), THUMB_MAX);
                            cache.insert(path.clone(), thumb);
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, path = %path.display(), "thumbnail failed");
                        }
                    }
```

Y en `texture_from_cache`, usar `first_frame()` y `dimensions()` de `LoadedImage`:

```rust
    fn texture_from_cache(&self, path: &std::path::Path) -> Option<(egui::TextureHandle, Vec2)> {
        let entry = self.cache.get(path)?;
        let texture = make_texture(&self.ctx, entry.first_frame());
        let size = entry.dimensions();
        Some((texture, Vec2::new(size.0 as f32, size.1 as f32)))
    }
```

- [ ] **Step 4: Actualizar `tests/integration.rs` (flujo 1)**

En `flujo_apertura_completo`, la imagen ahora es `LoadedImage`; el cache se puebla con `insert_loaded`:

```rust
    let image = load_image(nav.current_path().expect("imagen actual"))
        .expect("decodificar imagen sintética");
    let (w, h) = image.dimensions();
    assert_eq!((w, h), (64, 64), "la imagen sintética es 64x64");

    let cache = ImageCache::new(512);
    let result = cache.insert_loaded(target.clone(), image);
```

- [ ] **Step 5: Compilar y testear**

Run: `cargo check`
Expected: compila (el shim `insert` deja los benches/tests intactos).

Run: `cargo test -p sh_images --lib core::image_loader core::image_cache`
Expected: PASS (los tests existentes siguen pasando con el shim).

- [ ] **Step 6: Commit**

```bash
git add src/core/image_loader.rs src/core/image_cache.rs src/app.rs tests/integration.rs
git commit -m "feat(core): LoadedImage in loader and cache (static path)"
```

---

### Task 2: Decodificación GIF animado + selección de frame

**Files:**
- Modify: `src/core/image_loader.rs`
- Test: `cargo test -p sh_images --lib core::image_loader`

- [ ] **Step 1: Añadir la rama GIF y los métodos de frame**

Cambios en `src/core/image_loader.rs`:

1. Imports (sustituye el bloque `use image::...`):
```rust
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, GenericImageView, ImageFormat, ImageReader};
```

2. Constante y métodos nuevos dentro de `impl LoadedImage` (tras `first_frame`):
```rust
    /// Índice del frame activo para `elapsed` (bucle infinito sobre la
    /// duración total).
    pub fn frame_index_at(&self, elapsed: Duration) -> usize {
        match self {
            LoadedImage::Static(_) => 0,
            LoadedImage::Animated(anim) => {
                let total_ms = anim.total_duration.as_millis();
                if total_ms == 0 {
                    return 0;
                }
                let mut t = elapsed.as_millis() % total_ms;
                for (i, frame) in anim.frames.iter().enumerate() {
                    if t < frame.delay.as_millis() {
                        return i;
                    }
                    t -= frame.delay.as_millis();
                }
                anim.frames.len().saturating_sub(1)
            }
        }
    }

    /// Imagen del frame activo para `elapsed`.
    pub fn frame_at(&self, elapsed: Duration) -> &DynamicImage {
        match self {
            LoadedImage::Static(img) => img,
            LoadedImage::Animated(anim) => &anim.frames[self.frame_index_at(elapsed)].image,
        }
    }

    /// Tiempo restante hasta el próximo cambio de frame (para programar
    /// repaints). En `Static` devuelve una hora (no hay animación).
    pub fn time_to_next_frame(&self, elapsed: Duration) -> Duration {
        match self {
            LoadedImage::Static(_) => Duration::from_secs(3600),
            LoadedImage::Animated(anim) => {
                let total_ms = anim.total_duration.as_millis();
                if total_ms == 0 {
                    return MIN_FRAME_DELAY;
                }
                let t = elapsed.as_millis() % total_ms;
                let mut cum = 0u128;
                for frame in &anim.frames {
                    let d = frame.delay.as_millis();
                    if t < cum + d {
                        let remaining = (cum + d - t) as u64;
                        return Duration::from_millis(remaining.max(1));
                    }
                    cum += d;
                }
                anim.frames
                    .first()
                    .map(|f| f.delay)
                    .unwrap_or(MIN_FRAME_DELAY)
            }
        }
    }
```

3. Reemplazar `load_image` por la versión con rama GIF, y añadir los helpers:

```rust
/// Retardo mínimo de un frame animado; evita busy-loop de repaint con delay 0.
pub const MIN_FRAME_DELAY: Duration = Duration::from_millis(20);

/// Carga y decodifica una imagen desde el filesystem.
///
/// Devuelve `LoadedImage::Static` para formatos sin animación (o un GIF de un
/// solo frame) y `LoadedImage::Animated` para GIFs animados (frames ya
/// compuestos con sus retardos).
pub fn load_image(path: &Path) -> Result<LoadedImage> {
    let reader = ImageReader::open(path)?;
    let reader = reader.with_guessed_format()?;
    if reader.format() == Some(ImageFormat::Gif) {
        return load_animated_gif(path);
    }
    let image = reader.decode().map_err(map_image_error)?;
    Ok(LoadedImage::Static(image))
}

/// Decodifica un GIF animado completo.
///
/// Un GIF de un solo frame se devuelve como `Static` (no hay nada que animar).
fn load_animated_gif(path: &Path) -> Result<LoadedImage> {
    let file = std::fs::File::open(path)?;
    let decoder = GifDecoder::new(BufReader::new(file)).map_err(map_image_error)?;
    let mut frames = Vec::new();
    for frame in decoder.into_frames() {
        let frame = frame.map_err(map_image_error)?;
        let delay = clamp_delay(Duration::from(frame.delay()));
        frames.push(AnimatedFrame {
            image: DynamicImage::ImageRgba8(frame.into_buffer()),
            delay,
        });
    }
    match frames.len() {
        0 => Err(ShImagesError::Decode("gif sin frames".to_string())),
        1 => Ok(LoadedImage::Static(frames.remove(0).image)),
        _ => {
            let total_duration = frames.iter().map(|f| f.delay).sum();
            Ok(LoadedImage::Animated(AnimatedImage {
                frames,
                total_duration,
            }))
        }
    }
}

/// Clamp del retardo: un delay 0 (muy común en GIFs) no debe provocar un
/// repaint por frame infinito.
fn clamp_delay(d: Duration) -> Duration {
    if d < MIN_FRAME_DELAY {
        MIN_FRAME_DELAY
    } else {
        d
    }
}
```

- [ ] **Step 2: Escribir los tests que fallan**

En el `mod tests` de `image_loader.rs`, añade el helper de fixture y los tests:

```rust
    /// Genera un GIF animado sintético con `delays_ms` retardos por frame.
    fn make_animated_gif(dir: &Path, delays_ms: &[u64]) -> std::path::PathBuf {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame};
        let path = dir.join("animated.gif");
        let mut out = std::fs::File::create(&path).expect("crear gif");
        let mut encoder = GifEncoder::new(&mut out);
        let frames = delays_ms.iter().map(|&ms| {
            let buf =
                image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
            Frame::from_parts(
                buf,
                0,
                0,
                Delay::from_saturating_duration(Duration::from_millis(ms)),
            )
        });
        encoder.encode_frames(frames).expect("encodificar gif");
        path
    }

    #[test]
    fn read_animated_gif_yields_frames_and_total() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100, 200]);
        let loaded = load_image(&path).unwrap();
        let LoadedImage::Animated(anim) = &loaded else {
            panic!("gif de 2 frames debe ser Animated");
        };
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(anim.frames[0].delay, Duration::from_millis(100));
        assert_eq!(anim.frames[1].delay, Duration::from_millis(200));
        assert_eq!(anim.total_duration, Duration::from_millis(300));
        assert!(loaded.is_animated());
        assert_eq!(loaded.first_frame().dimensions(), (4, 4));
    }

    #[test]
    fn single_frame_gif_is_static() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100]);
        let loaded = load_image(&path).unwrap();
        assert!(!loaded.is_animated());
        assert!(matches!(loaded, LoadedImage::Static(_)));
    }

    #[test]
    fn zero_delay_is_clamped_to_minimum() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[0, 0]);
        let loaded = load_image(&path).unwrap();
        let LoadedImage::Animated(anim) = &loaded else {
            panic!("gif de 2 frames debe ser Animated");
        };
        assert_eq!(anim.frames[0].delay, MIN_FRAME_DELAY);
        assert_eq!(anim.frames[1].delay, MIN_FRAME_DELAY);
    }

    #[test]
    fn corrupt_gif_returns_decode_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupt.gif");
        std::fs::write(&path, b"GIF89a not really a gif").expect("escribir");
        let err = load_image(&path).expect_err("gif corrupto da error");
        assert!(matches!(err, ShImagesError::Decode(_) | ShImagesError::Io(_)));
    }

    #[test]
    fn frame_selection_wraps_around_total() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100, 200]);
        let loaded = load_image(&path).unwrap();
        assert_eq!(loaded.frame_index_at(Duration::from_millis(0)), 0);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(99)), 0);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(100)), 1);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(299)), 1);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(300)), 0);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(450)), 1); // 450%300=150
    }

    #[test]
    fn time_to_next_frame_is_remaining_of_current() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100, 200]);
        let loaded = load_image(&path).unwrap();
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(0)),
            Duration::from_millis(100)
        );
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(50)),
            Duration::from_millis(50)
        );
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(100)),
            Duration::from_millis(200)
        );
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(250)),
            Duration::from_millis(50)
        );
    }
```

- [ ] **Step 3: Ejecutar y ver que pasan**

Run: `cargo test -p sh_images --lib core::image_loader`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/core/image_loader.rs
git commit -m "feat(core): decode animated GIF with frame selection"
```

---

### Task 3: Reproducción en `app.rs` (`AnimState` + `tick_animation`)

**Files:**
- Modify: `src/app.rs`
- Test: `cargo check` + QA manual opcional

- [ ] **Step 1: Imports y struct `AnimState`**

En `src/app.rs`:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
```

Añadir el struct de estado (tras `LoadEvent`):

```rust
/// Estado de reproducción del GIF actual (None si la imagen es estática).
struct AnimState {
    started: Instant,
    current_frame: usize,
}
```

- [ ] **Step 2: Campo `anim` en `ShImagesApp`**

```rust
    /// Estado de reproducción de la animación del GIF actual.
    anim: Option<AnimState>,
```

Inicialízalo en `Self { ... }` de `new`:

```rust
            info_panel: InfoPanelState::default(),
            anim: None,
```

- [ ] **Step 3: Fijar `anim` en `apply_decoded`**

Al final de `apply_decoded` (tras `self.request_exif(path);`):

```rust
        let animated = self.cache.get(path).map(|e| e.is_animated()).unwrap_or(false);
        self.anim = if animated {
            Some(AnimState {
                started: Instant::now(),
                current_frame: 0,
            })
        } else {
            None
        };
```

- [ ] **Step 4: Método `tick_animation`**

Tras `poll_exif`:

```rust
    /// Avanza el GIF actual: reconstruye la textura cuando cambia el frame
    /// activo y programa el repaint para el próximo cambio.
    fn tick_animation(&mut self) {
        let Some(anim) = self.anim.as_mut() else {
            return;
        };
        let Some(path) = self
            .navigation
            .as_ref()
            .and_then(|n| n.current_path())
            .cloned()
        else {
            return;
        };
        let Some(entry) = self.cache.get(&path) else {
            return;
        };
        if !entry.is_animated() {
            return;
        }
        let elapsed = anim.started.elapsed();
        let idx = entry.frame_index_at(elapsed);
        if idx != anim.current_frame {
            self.texture = Some(make_texture(&self.ctx, entry.frame_at(elapsed)));
            anim.current_frame = idx;
        }
        let wait = entry.time_to_next_frame(elapsed);
        self.ctx.request_repaint_after(wait);
    }
```

- [ ] **Step 5: Llamar `tick_animation` en `ui()`**

En `ui()`, tras `self.poll_exif();`:

```rust
        self.tick_animation();
```

- [ ] **Step 6: Verificar**

Run: `cargo check`
Expected: limpio. `cargo clippy -- -D warnings` limpio.

QA manual (opcional): `cargo run`, abrir un GIF animado → debe reproducirse en bucle.

- [ ] **Step 7: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): play animated GIFs with frame texture rebuild"
```

---

### Task 4: `core/slideshow.rs` — lógica pura del intervalo

**Files:**
- Create: `src/core/slideshow.rs`
- Modify: `src/core/mod.rs`
- Test: `cargo test -p sh_images --lib core::slideshow`

- [ ] **Step 1: Escribir los tests que fallan**

Crea `src/core/slideshow.rs` con el módulo (sin impl todavía):

```rust
//! Lógica pura del slideshow: intervalos y límites.
//!
//! `core/` no depende de la UI; `app.rs` orquesta el avance con estas funciones.

use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_interval_is_five_seconds() {
        assert_eq!(default_interval(), Duration::from_secs(5));
    }

    #[test]
    fn faster_halves_interval() {
        assert_eq!(
            faster(Duration::from_secs(5)),
            Duration::from_millis(2500)
        );
    }

    #[test]
    fn faster_clamps_at_one_second() {
        assert_eq!(faster(Duration::from_secs(1)), Duration::from_secs(1));
        assert_eq!(faster(Duration::from_secs(2)), Duration::from_secs(1));
    }

    #[test]
    fn slower_doubles_interval() {
        assert_eq!(slower(Duration::from_secs(5)), Duration::from_secs(10));
    }

    #[test]
    fn slower_clamps_at_sixty_seconds() {
        assert_eq!(slower(Duration::from_secs(60)), Duration::from_secs(60));
        assert_eq!(slower(Duration::from_secs(30)), Duration::from_secs(60));
    }

    #[test]
    fn elapsed_reached_compares_correctly() {
        assert!(elapsed_reached(Duration::from_secs(5), Duration::from_secs(5)));
        assert!(!elapsed_reached(Duration::from_secs(4), Duration::from_secs(5)));
    }
}
```

- [ ] **Step 2: Ejecutar y ver que falla**

Run: `cargo test -p sh_images --lib core::slideshow`
Expected: FAIL — `default_interval`, `faster`, `slower`, `elapsed_reached` no definidos.

- [ ] **Step 3: Implementar**

Añade tras el import en `src/core/slideshow.rs`:

```rust
/// Intervalo mínimo del slideshow.
pub const MIN_INTERVAL: Duration = Duration::from_secs(1);
/// Intervalo máximo del slideshow.
pub const MAX_INTERVAL: Duration = Duration::from_secs(60);
/// Intervalo por defecto.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

/// Intervalo por defecto del slideshow (5 s).
pub fn default_interval() -> Duration {
    DEFAULT_INTERVAL
}

/// Acelera el slideshow: divide el intervalo por dos, sin bajar de 1 s.
pub fn faster(interval: Duration) -> Duration {
    (interval / 2).max(MIN_INTERVAL)
}

/// Ralentiza el slideshow: duplica el intervalo, sin superar 60 s.
pub fn slower(interval: Duration) -> Duration {
    (interval * 2).min(MAX_INTERVAL)
}

/// `true` si `elapsed` ya superó `interval` (toca avanzar).
pub fn elapsed_reached(elapsed: Duration, interval: Duration) -> bool {
    elapsed >= interval
}
```

- [ ] **Step 4: Registrar el módulo**

En `src/core/mod.rs`, añade tras `pub mod shortcuts;`:

```rust
pub mod slideshow;
```

- [ ] **Step 5: Ejecutar y ver que pasa**

Run: `cargo test -p sh_images --lib core::slideshow`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/core/slideshow.rs src/core/mod.rs
git commit -m "feat(core): slideshow interval logic (pure)"
```

---

### Task 5: Acciones `ToggleSlideshow`/`Faster`/`Slower` + atajos + toolbar

**Files:**
- Modify: `src/core/actions.rs`
- Modify: `src/core/shortcuts.rs`
- Modify: `src/ui/shortcut_dialog.rs`
- Modify: `src/ui/toolbar.rs`
- Test: `cargo test -p sh_images --lib core::actions core::shortcuts`

- [ ] **Step 1: `src/core/actions.rs`**

1. Añadir las variantes al enum, tras `ToggleInfo`:

```rust
    /// Iniciar o detener el slideshow automático.
    ToggleSlideshow,
    /// Acelerar el slideshow.
    SlideshowFaster,
    /// Ralentizar el slideshow.
    SlideshowSlower,
```

2. Añadir a `label()` (tras `ToggleInfo`):

```rust
            Action::ToggleSlideshow => "Iniciar/detener slideshow",
            Action::SlideshowFaster => "Slideshow más rápido",
            Action::SlideshowSlower => "Slideshow más lento",
```

3. Añadir a `default_shortcut()` (tras `ToggleInfo`):

```rust
            Action::ToggleSlideshow => KeyBinding::new(KeyCode::KeyF5, Modifiers::None),
            Action::SlideshowFaster => KeyBinding::new(KeyCode::Comma, Modifiers::None),
            Action::SlideshowSlower => KeyBinding::new(KeyCode::Period, Modifiers::None),
```

4. `all()` pasa a `[Action; 14]` y añade las tres tras `Action::ToggleInfo`:

```rust
    pub fn all() -> [Action; 14] {
        [
            Action::Open,
            Action::Prev,
            Action::Next,
            Action::RotateCw,
            Action::RotateCcw,
            Action::Fit,
            Action::Fullscreen,
            Action::ToggleTheme,
            Action::ToggleSidebar,
            Action::ToggleInfo,
            Action::ToggleSlideshow,
            Action::SlideshowFaster,
            Action::SlideshowSlower,
            Action::EditShortcuts,
        ]
    }
```

5. Tests: en `label_is_stable_and_descriptive` añade:

```rust
        assert_eq!(
            Action::ToggleSlideshow.label(),
            "Iniciar/detener slideshow"
        );
        assert_eq!(Action::SlideshowFaster.label(), "Slideshow más rápido");
        assert_eq!(Action::SlideshowSlower.label(), "Slideshow más lento");
```

Renombra `all_returns_eleven_actions` a `all_returns_fourteen_actions` y cambia los dos `11` por `14`:

```rust
    #[test]
    fn all_returns_fourteen_actions() {
        let all = Action::all();
        assert_eq!(all.len(), 14);
        let unique: std::collections::HashSet<_> = all.into_iter().collect();
        assert_eq!(unique.len(), 14, "sin variantes duplicadas");
    }
```

- [ ] **Step 2: `src/core/shortcuts.rs`**

1. Añadir al enum `KeyCode` (tras `F11`):

```rust
    KeyF5,
    Comma,
    Period,
```

2. Añadir a `to_str()` (tras `F11`):

```rust
            KeyCode::KeyF5 => "F5",
            KeyCode::Comma => ",",
            KeyCode::Period => ".",
```

3. Añadir a `from_str()` (tras `F11`):

```rust
            "F5" => Some(KeyCode::KeyF5),
            "," => Some(KeyCode::Comma),
            "." => Some(KeyCode::Period),
```

4. Tests: en `defaults_has_one_entry_per_action` cambia `11` por `14`. En `default_shortcuts_match_spec`, tras la línea de `ToggleInfo`, añade:

```rust
        assert_eq!(map.get(Action::ToggleSlideshow).unwrap().to_string(), "F5");
        assert_eq!(map.get(Action::SlideshowFaster).unwrap().to_string(), ",");
        assert_eq!(map.get(Action::SlideshowSlower).unwrap().to_string(), ".");
```

- [ ] **Step 3: `src/ui/shortcut_dialog.rs`**

En `keybinding_from_egui`, tras el mapeo de `F11`:

```rust
        egui::Key::F5 => KeyCode::KeyF5,
        egui::Key::Comma => KeyCode::Comma,
        egui::Key::Period => KeyCode::Period,
```

- [ ] **Step 4: `src/ui/toolbar.rs`**

1. La firma pasa a:

```rust
pub fn show(
    ui: &mut egui::Ui,
    shortcuts: &ShortcutMap,
    theme_name: &str,
    is_fullscreen: bool,
    slideshow_active: bool,
) -> Option<Action> {
```

2. Botón tras el de `Info`:

```rust
            if toolbar_button(ui, "▶", Action::ToggleSlideshow, shortcuts) {
                clicked = Some(Action::ToggleSlideshow);
            }
```

3. Indicador a la derecha (junto al de fullscreen):

```rust
                if slideshow_active {
                    ui.colored_label(egui::Color32::GREEN, "[▶] Slideshow");
                }
```

- [ ] **Step 5: Actualizar la llamada en `app.rs`**

```rust
        let action = toolbar::show(
            ui,
            &self.shortcuts,
            &self.settings.theme,
            self.is_fullscreen,
            self.slideshow_active,
        );
```

(El campo `slideshow_active` se añade en la Task 7; mientras tanto usa `false` para que compile: sustituye `self.slideshow_active` por `false` en esta llamada y reviértelo en la Task 7.)

- [ ] **Step 6: Ejecutar y regenerar snapshots**

Run: `cargo test -p sh_images --lib core::actions core::shortcuts`
Expected: FALLAN `snapshot_default_shortcuts_map` y `snapshot_default_keybinding_strings` (nuevos atajos).

Regenerar y revisar el diff (solo deben aparecer filas `toggle_slideshow`/`F5`, `slideshow_faster`/`,` y `slideshow_slower`/`.`):

```bash
$env:INSTA_UPDATE="always"; cargo test -p sh_images --lib core::shortcuts
```

Revisa el diff en `src/core/snapshots/` antes de commitear.

- [ ] **Step 7: Commit**

```bash
git add src/core/actions.rs src/core/shortcuts.rs src/ui/shortcut_dialog.rs src/ui/toolbar.rs src/app.rs src/core/snapshots
git commit -m "feat: add slideshow actions and shortcuts (F5, comma, period)"
```

---

### Task 6: `settings.slideshow_interval_secs`

**Files:**
- Modify: `src/config/settings.rs`
- Modify: `tests/integration.rs`
- Test: `cargo test -p sh_images --lib config::settings`

- [ ] **Step 1: Escribir los tests que fallan**

En `mod tests` de `src/config/settings.rs`:

```rust
    #[test]
    fn slideshow_interval_defaults_to_five() {
        let s = Settings::default();
        assert_eq!(s.slideshow_interval_secs, 5);
    }

    #[test]
    fn toml_without_new_field_migrates_to_default() {
        let content = "cache_memory_limit_mb = 256\ntheme = \"light\"\n";
        let s: Settings = toml::from_str(content).expect("deserializar");
        assert_eq!(s.slideshow_interval_secs, 5);
    }

    #[test]
    fn slideshow_interval_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let settings = Settings {
            cache_memory_limit_mb: 256,
            theme: "light".to_string(),
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 10,
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded, settings);
    }
```

- [ ] **Step 2: Ejecutar y ver que falla**

Run: `cargo test -p sh_images --lib config::settings`
Expected: FAIL — el campo no existe.

- [ ] **Step 3: Añadir el campo**

En `src/config/settings.rs`, tras `shortcuts`:

```rust
    /// Intervalo del slideshow en segundos (default: 5).
    #[serde(default)]
    pub slideshow_interval_secs: u64,
```

Y en `impl Default for Settings`:

```rust
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 5,
```

- [ ] **Step 4: Actualizar los tests existentes**

En `default_settings_match_plan_values` añade:

```rust
        assert_eq!(s.slideshow_interval_secs, 5);
```

En `loading_missing_file_creates_defaults_and_persists` añade:

```rust
        assert!(on_disk.contains("slideshow_interval_secs = 5"));
```

En `save_then_load_roundtrips`, el literal de `Settings` gana el campo:

```rust
        let settings = Settings {
            cache_memory_limit_mb: 256,
            theme: "light".to_string(),
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 5,
        };
```

En `save_overwrites_existing_file`, el segundo literal:

```rust
        let second = Settings {
            cache_memory_limit_mb: 128,
            theme: "dark".to_string(),
            shortcuts: ShortcutMap::defaults(),
            slideshow_interval_secs: 5,
        };
```

- [ ] **Step 5: Actualizar `tests/integration.rs` (flujo 5)**

En `flujo_configuracion_persistencia`, el literal `modified`:

```rust
    let modified = Settings {
        cache_memory_limit_mb: 256,
        theme: "light".to_string(),
        shortcuts: ShortcutMap::defaults(),
        slideshow_interval_secs: 5,
    };
```

- [ ] **Step 6: Ejecutar y ver que pasa**

Run: `cargo test -p sh_images --lib config::settings`
Expected: PASS. Run: `cargo test -p sh_images --test integration`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/config/settings.rs tests/integration.rs
git commit -m "feat(config): persist slideshow interval in settings"
```

---

### Task 7: Wiring del slideshow en `app.rs`

**Files:**
- Modify: `src/app.rs`
- Test: `cargo check` + `cargo clippy -- -D warnings`

- [ ] **Step 1: Imports y campos**

```rust
use crate::core::shortcuts::ShortcutMap;
use crate::core::slideshow;
use crate::core::thumb_queue::ThumbQueue;
```

Campos de `ShImagesApp` (tras `info_panel`):

```rust
    /// Si el slideshow automático está activo.
    slideshow_active: bool,
    /// Intervalo actual del slideshow.
    slideshow_interval: Duration,
    /// Última vez que el slideshow avanzó de imagen.
    slideshow_last_advance: Instant,
```

- [ ] **Step 2: Inicializar en `new`**

Antes de `Self { ... }`:

```rust
        let slideshow_interval = Duration::from_secs(settings.slideshow_interval_secs.max(1));
```

Y en `Self { ... }` (tras `anim: None,`):

```rust
            slideshow_active: false,
            slideshow_interval,
            slideshow_last_advance: Instant::now(),
```

- [ ] **Step 3: Métodos de slideshow**

Tras `toggle_sidebar`:

```rust
    /// Alterna el slideshow y reinicia su contador.
    fn toggle_slideshow(&mut self) {
        self.slideshow_active = !self.slideshow_active;
        self.slideshow_last_advance = Instant::now();
        self.ctx.request_repaint();
        tracing::info!(active = self.slideshow_active, "slideshow toggled");
    }

    /// Ajusta la velocidad del slideshow y persiste el intervalo.
    fn change_slideshow_speed(&mut self, faster: bool) {
        self.slideshow_interval = if faster {
            slideshow::faster(self.slideshow_interval)
        } else {
            slideshow::slower(self.slideshow_interval)
        };
        self.settings.slideshow_interval_secs = self.slideshow_interval.as_secs().max(1);
        if let Ok(path) = settings_path() {
            if let Err(e) = self.settings.save(&path) {
                tracing::warn!(error = %e, "failed to persist slideshow interval");
            }
        }
        self.slideshow_last_advance = Instant::now();
        tracing::info!(
            interval_secs = self.settings.slideshow_interval_secs,
            "slideshow speed changed"
        );
    }

    /// Avanza una imagen sin pausar el slideshow (auto-avance).
    fn advance_slideshow(&mut self) {
        self.slideshow_last_advance = Instant::now();
        if let Some(nav) = &mut self.navigation {
            nav.next();
            if let Some(path) = nav.current_path().cloned() {
                self.start_load(path);
            }
        }
    }

    /// Detiene el slideshow por interacción manual del usuario.
    fn pause_slideshow(&mut self) {
        if self.slideshow_active {
            self.slideshow_active = false;
            tracing::info!("slideshow paused by user interaction");
        }
    }
```

- [ ] **Step 4: Pausa en `dispatch` (Prev/Next) y en los arm nuevos**

```rust
            Action::Prev => {
                self.pause_slideshow();
                self.navigate(-1);
            }
            Action::Next => {
                self.pause_slideshow();
                self.navigate(1);
            }
```

Y añadir los arm nuevos (tras `ToggleInfo`):

```rust
            Action::ToggleSlideshow => self.toggle_slideshow(),
            Action::SlideshowFaster => self.change_slideshow_speed(true),
            Action::SlideshowSlower => self.change_slideshow_speed(false),
```

- [ ] **Step 5: Pausa en `navigate_to` y en el zoom**

Al inicio de `navigate_to`:

```rust
    fn navigate_to(&mut self, index: usize) {
        self.pause_slideshow();
        let Some(nav) = &mut self.navigation else {
            return;
        };
```

En `ui()`, donde se maneja el zoom:

```rust
                    let resp = viewer::show(ui, texture, &mut self.transform);
                    if resp.zoomed {
                        self.user_interacted = true;
                        self.pause_slideshow();
                    }
```

- [ ] **Step 6: Auto-avance en `ui()`**

Tras `self.tick_animation();`:

```rust
        if self.slideshow_active && self.navigation.is_some() {
            if slideshow::elapsed_reached(
                self.slideshow_last_advance.elapsed(),
                self.slideshow_interval,
            ) {
                self.advance_slideshow();
            }
            self.ctx.request_repaint_after(self.slideshow_interval);
        }
```

- [ ] **Step 7: Restaurar `self.slideshow_active` en la llamada a `toolbar::show`**

Si en la Task 5 dejaste `false`, cámbialo por `self.slideshow_active`.

- [ ] **Step 8: Verificar**

Run: `cargo check`
Expected: limpio.

Run: `cargo clippy -- -D warnings`
Expected: sin warnings.

QA manual: `cargo run` → abrir carpeta → `F5` inicia el slideshow; `,`/`.` ajustan velocidad; ←/→ o zoom lo pausan.

- [ ] **Step 9: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): wire slideshow auto-advance and pause on interaction"
```

---

### Task 8: Tests de integración (flujos 8 y 9)

**Files:**
- Modify: `tests/common/mod.rs`
- Modify: `tests/integration.rs`
- Test: `cargo test -p sh_images --test integration`

- [ ] **Step 1: Helper `make_animated_gif` en `tests/common/mod.rs`**

Imports (añadir a los existentes):

```rust
use std::time::Duration;

use image::codecs::gif::GifEncoder;
use image::{Delay, Frame};
```

Helper (tras `gif_path`):

```rust
/// Guarda un GIF animado sintético con `delays_ms` retardos por frame.
pub fn make_animated_gif(dir: &Path, delays_ms: &[u64]) -> PathBuf {
    let path = dir.join("animated.gif");
    let mut out = fs::File::create(&path).expect("crear gif");
    let mut encoder = GifEncoder::new(&mut out);
    let frames = delays_ms.iter().map(|&ms| {
        let buf = gradient_image(8, 8).to_rgba8();
        Frame::from_parts(
            buf,
            0,
            0,
            Delay::from_saturating_duration(Duration::from_millis(ms)),
        )
    });
    encoder.encode_frames(frames).expect("encodificar gif");
    path
}
```

- [ ] **Step 2: Flujos 8 y 9 en `tests/integration.rs`**

Imports (añadir):

```rust
use std::time::Duration;

use sh_images::core::image_loader::{load_image, LoadedImage};
use sh_images::core::slideshow;
```

Tests (tras `flujo_exif`):

```rust
/// Flujo 8 — GIF animado: cargar, verificar frames/retardos y selección por
/// tiempo; un GIF corrupto no crashea.
#[test]
fn flujo_gif_animado() {
    let dir = tempfile::tempdir().expect("tempdir");

    let gif = make_animated_gif(dir.path(), &[100, 200]);
    let loaded = load_image(&gif).expect("gif válido");
    let LoadedImage::Animated(anim) = &loaded else {
        panic!("gif de 2 frames debe ser Animated");
    };
    assert_eq!(anim.frames.len(), 2);
    assert_eq!(anim.frames[0].delay, Duration::from_millis(100));
    assert_eq!(anim.frames[1].delay, Duration::from_millis(200));
    assert_eq!(anim.total_duration, Duration::from_millis(300));

    assert_eq!(loaded.frame_index_at(Duration::from_millis(0)), 0);
    assert_eq!(loaded.frame_index_at(Duration::from_millis(150)), 1);
    assert_eq!(loaded.frame_index_at(Duration::from_millis(300)), 0);

    let corrupt = dir.path().join("corrupt.gif");
    std::fs::write(&corrupt, b"GIF89a not really a gif").expect("escribir");
    let err = load_image(&corrupt).expect_err("gif corrupto da error");
    assert!(
        matches!(
            err,
            sh_images::utils::errors::ShImagesError::Decode(_)
                | sh_images::utils::errors::ShImagesError::Io(_)
        ),
        "gif corrupto no crashea"
    );
}

/// Flujo 9 — Slideshow: límites del intervalo y default de 5 s.
#[test]
fn flujo_slideshow_interval() {
    assert_eq!(slideshow::default_interval(), Duration::from_secs(5));
    assert_eq!(
        slideshow::faster(Duration::from_secs(5)),
        Duration::from_millis(2500)
    );
    assert_eq!(slideshow::slower(Duration::from_secs(5)), Duration::from_secs(10));
    assert_eq!(slideshow::faster(Duration::from_secs(1)), Duration::from_secs(1));
    assert_eq!(
        slideshow::slower(Duration::from_secs(60)),
        Duration::from_secs(60)
    );
    assert!(slideshow::elapsed_reached(
        Duration::from_secs(5),
        Duration::from_secs(5)
    ));
    assert!(!slideshow::elapsed_reached(
        Duration::from_secs(4),
        Duration::from_secs(5)
    ));
}
```

- [ ] **Step 3: Actualizar import en `tests/integration.rs`**

En el bloque `use common::{...}` añade `make_animated_gif`:

```rust
use common::{
    copy_fixture, corrupt_png_path, empty_png_path, gif_path, make_animated_gif,
    make_folder_with_images, make_folder_with_rect_images,
};
```

- [ ] **Step 4: Ejecutar y ver que pasa**

Run: `cargo test -p sh_images --test integration`
Expected: PASS (9 flujos).

- [ ] **Step 5: Commit**

```bash
git add tests/common/mod.rs tests/integration.rs
git commit -m "test: add integration flows for animated GIF and slideshow"
```

---

### Task 9: Documentación (ADR-010, ADR-011, CHANGELOG, Plan.md)

**Files:**
- Modify: `docs/ARCHITECTURE.md`
- Modify: `CHANGELOG.md`
- Modify: `Plan.md`

- [ ] **Step 1: ADR-010 en `docs/ARCHITECTURE.md`**

Append:

```markdown
## ADR-010: Imagen unificada `LoadedImage` y GIF animado

- **Contexto:** `ImageCache` guardaba una `DynamicImage` por path; un GIF
  animado son N frames con retardos. `image::open` solo devuelve el primer frame.
- **Decisión:** `core/image_loader.rs` expone `LoadedImage::{ Static, Animated }`
  (`AnimatedImage` = frames compuestos + `total_duration`). `load_image` usa
  `ImageReader` + `GifDecoder` para GIF (retardos clampados a un mínimo de 20 ms)
  y `ImageCache` guarda `LoadedImage` contabilizando la suma de frames. La app
  guarda `AnimState` y reconstruye la textura al cambiar de frame
  (`frame_index_at` / `time_to_next_frame` puros). `insert(DynamicImage)` se
  mantiene como shim de `insert_loaded`.
- **Consecuencias:** Miniaturas y pre-carga usan `first_frame()`; el viewer no
  cambia (pinta una textura; la rotación mesh aplica a cualquier frame). Un GIF
  de un solo frame se trata como `Static`.
- **Alternativas:** cache animado separado (dos pipelines), decodificación bajo
  demanda en el UI thread (viola §7.1).
```

- [ ] **Step 2: ADR-011 en `docs/ARCHITECTURE.md`**

Append:

```markdown
## ADR-011: Slideshow automático con intervalo configurable

- **Contexto:** Fase 4 pide avanzar automáticamente por la carpeta.
- **Decisión:** `core/slideshow.rs` (puro) define `default_interval()` (5 s),
  `faster`/`slower` (límites 1–60 s) y `elapsed_reached`. Tres acciones:
  `ToggleSlideshow` (F5), `SlideshowFaster` (","), `SlideshowSlower` (".").
  `settings.slideshow_interval_secs` persiste el intervalo. En `app.rs`, el
  auto-avance usa `advance_slideshow()` (no pasa por dispatch y no se pausa a sí
  mismo); la navegación manual (←/→, miniatura) y el zoom pausan el slideshow.
- **Consecuencias:** La lógica de velocidad es testeable en `core/`; el intervalo
  sobrevive reinicios vía settings; el slideshow se despierta con
  `request_repaint_after`.
- **Alternativas:** intervalo fijo sin configuración (menos flexible), sin
  acciones de velocidad (depender solo de settings).
```

- [ ] **Step 3: `CHANGELOG.md`**

Añade bajo la entrada "Fase 4 — Metadatos EXIF" (o reemplaza la sección Fase 4 por una única):

```markdown
## Fase 4 — Metadatos EXIF, GIF animado y slideshow

- Lectura de metadatos EXIF (JPEG/TIFF) en `core/exif.rs` con `kamadak-exif`.
- Panel derecho de información (`ui::info_panel`) con campos curados
  (Fabricante, Modelo, Fecha, ISO, Apertura, Obturador, Focal, Orientación).
- Acción `ToggleInfo` (atajo `I`) para mostrar/ocultar el panel.
- Carga asíncrona de EXIF en segundo plano con cache por path.
- Reproducción de GIF animado en bucle (`LoadedImage::Animated`, retardos por
  frame, textura reconstruida al cambiar de frame).
- Slideshow automático: `ToggleSlideshow` (F5), `SlideshowFaster` (","),
  `SlideshowSlower` ("."); intervalo persistido en settings (default 5 s);
  pausa al navegar o hacer zoom.
```

- [ ] **Step 4: `Plan.md`**

En §4, Fase 4, marca todos los ítems:

```markdown
### Fase 4 — Metadatos Avanzados (Semana 5)
- [x] Lectura de metadatos EXIF
- [x] Panel lateral con info EXIF (cámara, ISO, apertura, fecha)
- [x] Soporte para GIF animado
- [x] Slideshow automático
- [x] **Tests**: extracción EXIF de múltiples formatos, manejo de archivos corruptos
```

- [ ] **Step 5: Commit**

```bash
git add docs/ARCHITECTURE.md CHANGELOG.md Plan.md
git commit -m "docs: ADR-010/011 GIF + slideshow, changelog and Plan.md"
```

---

### Task 10: QA final (AGENTS.md §5.1)

**Files:** ninguno

- [ ] **Step 1: Checklist de calidad**

Run:

```bash
cargo fmt
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test --release
```

Expected: todo pasa. Si algún snapshot quedó desactualizado, regenera con
`INSTA_UPDATE=always` y revisa el diff.

- [ ] **Step 2: sin `.unwrap()`/`.expect()` en producción**

Run:

```bash
rg -n "\.unwrap\(\)|\.expect\(" src
```

Expected: solo dentro de `#[cfg(test)]` y los `unwrap_or_else(|p| p.into_inner())`
de los locks. `request_exif` usa `.unwrap_or(false)` (no panic).

- [ ] **Step 3: QA manual documentado**

`cargo run`:
1. Abrir un GIF animado → se reproduce en bucle; zoom/fit/rotación funcionan.
2. Abrir una carpeta → `F5` inicia el slideshow; `,`/`.` cambian velocidad;
   `←`/`→` o zoom lo pausan; el intervalo persiste al reiniciar.
3. Abrir un PNG → se muestra estático; el panel de info sigue funcionando.

- [ ] **Step 4: Commit final**

```bash
git add -A
git commit -m "chore: final fmt/clippy pass for Fase 4 GIF + slideshow"
```

---

## Notas para el ejecutor

- **`ImageReader`:** `open(path)?` (io) y `with_guessed_format()?` (io) se
  convierten a `ShImagesError::Io`; `decode()` y los errores de `GifDecoder` se
  mapean con `map_image_error` (Unsupported → `UnsupportedFormat`, resto → `Decode`).
- **`GifDecoder`:** requiere `R: BufRead + Seek` para `into_frames()`; un
  `BufReader<File>` cumple ambos. `Frame::into_buffer() -> RgbaImage` y
  `Duration::from(Frame::delay())`.
- **Fixtures GIF:** se generan en runtime con `GifEncoder` (AGENTS.md §8.2: no
  commitear binarios).
- **Shim `ImageCache::insert(DynamicImage)`:** los tests/benches existentes que
  insertan `DynamicImage` no se tocan; solo se añaden tests nuevos para
  `insert_loaded`.
- **Snapshots de atajos:** regenerar con `INSTA_UPDATE=always` y revisar el diff
  (nunca aceptar ciegamente, AGENTS.md §7).
