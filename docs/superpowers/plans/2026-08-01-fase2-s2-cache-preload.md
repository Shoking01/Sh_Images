# Fase 2 — Subproyecto 2: Integración del LRU Cache y Pre-carga — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Conectar el `ImageCache` (S1) al flujo de carga de `app.rs` con un canal único persistente, deduplicación in-flight y pre-carga de la imagen N±1, de modo que la navegación ←/→ reutilice imágenes decodificadas.

**Architecture:** Canal único `mpsc` persistente clonado a cada worker; `LoadEvent { path, result: Result<()> }` (la imagen NO viaja por el canal — la cache es la única fuente). `start_load` tiene 3 ramas: cache hit (textura inmediata), in-flight (no-op), miss (spawn worker). `poll_loader` drena el canal y solo actúa si el path es el actual. La pre-carga usa `core/preload.rs::preload_targets` (función pura). Adiciones mínimas: `ImageCache::contains` y `Navigation::neighbor_paths`.

**Tech Stack:** `std::collections::HashSet`, `std::sync::{Arc, Mutex, mpsc}`, API existente de `image_cache.rs` (sin cambios). Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-08-01-fase2-s2-cache-preload-design.md`

---

## File Structure

```
src/core/image_cache.rs   # MODIFICAR — añadir ImageCache::contains (+ tests)
src/core/navigation.rs    # MODIFICAR — añadir Navigation::neighbor_paths (+ tests)
src/core/preload.rs       # CREAR — preload_targets (+ tests)
src/core/mod.rs           # MODIFICAR — registrar módulo preload
src/app.rs                # REWRITE — integrar cache + canal único + pre-carga
```

No se toca `Cargo.toml` (sin dependencias nuevas).

---

## Task 1: `ImageCache::contains` — chequeo sin reordenar LRU ni contar hits

**Files:**
- Modify: `src/core/image_cache.rs` (tests al final del módulo + método en `impl ImageCache`)

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al final del bloque `#[cfg(test)]` de `src/core/image_cache.rs` (tras el test `hit_ratio_tracks_hits_and_misses`):

```rust
    #[test]
    fn contains_is_true_for_cached_path() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(cache.contains(Path::new("a.png")));
    }

    #[test]
    fn contains_is_false_for_missing_path() {
        let cache = ImageCache::new(1);
        assert!(!cache.contains(Path::new("a.png")));
    }

    #[test]
    fn contains_does_not_reorder_lru() {
        let cache = ImageCache::new(1); // 1 MiB = 4 × 256 KiB
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        // Solo preguntar por "a" (la más vieja) NO debe moverla a MRU.
        assert!(cache.contains(Path::new("a.png")));
        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        // Sigue evictándose a (LRU), no b.
        assert_eq!(res.evicted_keys, vec![PathBuf::from("a.png")]);
    }

    #[test]
    fn contains_does_not_count_hits() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(cache.contains(Path::new("a.png")));
        assert!(cache.contains(Path::new("a.png")));
        assert_eq!(cache.hit_ratio(), 0.0);
    }
```

- [ ] **Step 2: Ejecutar los tests para verificar que fallan**

Run: `cargo test --lib core::image_cache::tests::contains_`
Expected: FAIL — error de compilación: `no method named 'contains' found for struct 'ImageCache'`.

- [ ] **Step 3: Implementar `contains`**

Añadir en `impl ImageCache` (justo después de `get`, líneas ~267):

```rust
    /// `true` si `path` está en el cache, sin reordenar la lista LRU ni contar hits.
    ///
    /// A diferencia de `get`, no marca la entrada como recién usada y no altera
    /// `hit_ratio`: sirve para "solo preguntar si está" (p.ej. planificar pre-carga).
    pub fn contains(&self, path: &Path) -> bool {
        self.lock().map.contains_key(path)
    }
```

- [ ] **Step 4: Ejecutar los tests para verificar que pasan**

Run: `cargo test --lib core::image_cache`
Expected: PASS — 21 tests (17 previos + 4 nuevos de `contains`).

- [ ] **Step 5: Commit**

```bash
git add src/core/image_cache.rs
git commit -m "feat: add ImageCache::contains without LRU reorder or hit counting"
```

---

## Task 2: `Navigation::neighbor_paths` — accesor circular prev/next

**Files:**
- Modify: `src/core/navigation.rs`

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al final del bloque `#[cfg(test)]` de `src/core/navigation.rs` (tras `next_on_empty_is_noop`):

```rust
    #[test]
    fn neighbor_paths_returns_prev_and_next() {
        let (_d, folder) = setup_folder(); // images: a.jpg, b.png, d.JPG
        let nav = Navigation::from_folder(&folder.join("b.png"), SUPPORTED_EXTENSIONS).unwrap();
        let [prev, next] = nav.neighbor_paths();
        assert_eq!(prev.unwrap(), &folder.join("a.jpg"));
        assert_eq!(next.unwrap(), &folder.join("d.JPG"));
    }

    #[test]
    fn neighbor_paths_single_image_returns_same_twice() {
        let dir = tempdir().unwrap();
        let only = dir.path().join("only.png");
        fs::write(&only, b"x").unwrap();
        let nav = Navigation::from_folder(&only, SUPPORTED_EXTENSIONS).unwrap();
        let [prev, next] = nav.neighbor_paths();
        assert_eq!(prev.unwrap(), &only);
        assert_eq!(next.unwrap(), &only);
    }

    #[test]
    fn neighbor_paths_empty_list_returns_none() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nothing.png");
        let nav = Navigation::from_folder(&missing, SUPPORTED_EXTENSIONS).unwrap();
        let [prev, next] = nav.neighbor_paths();
        assert!(prev.is_none());
        assert!(next.is_none());
    }
```

- [ ] **Step 2: Ejecutar los tests para verificar que fallan**

Run: `cargo test --lib core::navigation::tests::neighbor_paths_`
Expected: FAIL — error de compilación: `no method named 'neighbor_paths' found`.

- [ ] **Step 3: Implementar `neighbor_paths`**

Añadir en `impl Navigation` (tras `current_path`, líneas ~69):

```rust
    /// Rutas previa y siguiente (circulares) respecto a la actual.
    ///
    /// `[None, None]` si la lista está vacía. Con una sola imagen, ambas
    /// referencias apuntan a la misma ruta. No muta el estado (a diferencia de
    /// `next()`/`prev()`).
    pub fn neighbor_paths(&self) -> [Option<&PathBuf>; 2] {
        let len = self.images.len();
        if len == 0 {
            return [None, None];
        }
        let prev = (self.current + len - 1) % len;
        let next = (self.current + 1) % len;
        [self.images.get(prev), self.images.get(next)]
    }
```

- [ ] **Step 4: Ejecutar los tests para verificar que pasan**

Run: `cargo test --lib core::navigation`
Expected: PASS — 12 tests (9 previos + 3 nuevos).

- [ ] **Step 5: Commit**

```bash
git add src/core/navigation.rs
git commit -m "feat: add Navigation::neighbor_paths circular prev/next accessor"
```

---

## Task 3: `core/preload.rs` — lógica pura de pre-carga

**Files:**
- Create: `src/core/preload.rs`
- Modify: `src/core/mod.rs` (registrar el módulo)

- [ ] **Step 1: Escribir los tests que fallan**

Crear `src/core/preload.rs` con solo el módulo de tests (el build fallará: `Navigation` es conocido pero `preload_targets` y `PRELOAD_DEPTH` aún no existen):

```rust
//! Pre-carga de imágenes adyacentes (lógica pura, sin I/O ni threads).
//!
//! `core/` no depende de `egui` (AGENTS.md §3.2). Implementación completa en el
//! siguiente paso: aquí solo viven los tests (TDD, fail-first).

use std::path::{Path, PathBuf};

use crate::core::navigation::Navigation;

#[cfg(test)]
mod tests {
    use super::*;

    fn nav(images: &[&str], current: usize) -> Navigation {
        Navigation {
            images: images.iter().map(|s| PathBuf::from(s)).collect(),
            current,
        }
    }

    fn never(_p: &Path) -> bool {
        false
    }

    #[test]
    fn preloads_prev_and_next_in_priority_order() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 2);
        let targets = preload_targets(&n, 1, never, never);
        assert_eq!(targets, vec![PathBuf::from("3.png"), PathBuf::from("1.png")]);
    }

    #[test]
    fn wraps_circularly() {
        let n = nav(&["0.png", "1.png", "2.png"], 0);
        let targets = preload_targets(&n, 1, never, never);
        assert_eq!(targets, vec![PathBuf::from("1.png"), PathBuf::from("2.png")]);
    }

    #[test]
    fn skips_cached_paths() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 1);
        let targets = preload_targets(&n, 1, |p| p == Path::new("2.png"), never);
        assert_eq!(targets, vec![PathBuf::from("0.png")]);
    }

    #[test]
    fn skips_in_flight_paths() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 1);
        let targets = preload_targets(&n, 1, never, |p| p == Path::new("2.png"));
        assert_eq!(targets, vec![PathBuf::from("0.png")]);
    }

    #[test]
    fn depth_two_preloads_four_in_order() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png", "4.png", "5.png"], 3);
        let targets = preload_targets(&n, 2, never, never);
        assert_eq!(
            targets,
            vec![
                PathBuf::from("4.png"),
                PathBuf::from("2.png"),
                PathBuf::from("5.png"),
                PathBuf::from("1.png"),
            ]
        );
    }

    #[test]
    fn skips_current_image() {
        let n = nav(&["0.png"], 0);
        assert!(preload_targets(&n, 3, never, never).is_empty());
    }

    #[test]
    fn no_duplicates_when_depth_exceeds_size() {
        let n = nav(&["0.png", "1.png", "2.png"], 1);
        let targets = preload_targets(&n, 5, never, never);
        assert_eq!(targets.len(), 2);
    }
}
```

Registrar el módulo en `src/core/mod.rs`:

```rust
pub mod preload;
```

(Insertar entre `pub mod navigation;` y `pub mod thumbnail_gen;`.)

- [ ] **Step 2: Ejecutar los tests para verificar que fallan**

Run: `cargo test --lib core::preload`
Expected: FAIL — error de compilación: `cannot find function 'preload_targets' in this scope` y `cannot find constant 'PRELOAD_DEPTH'` (el test no los usa aún, pero el módulo queda sin ellos).

- [ ] **Step 3: Implementar `preload_targets` y `PRELOAD_DEPTH`**

Reemplazar `src/core/preload.rs` completo por:

```rust
//! Pre-carga de imágenes adyacentes (lógica pura, sin I/O ni threads).
//!
//! `core/` no depende de `egui` (AGENTS.md §3.2). La decisión de qué paths
//! precargar es una función pura y testeable; el orquestado (spawnear workers)
//! vive en `app.rs`.

use std::path::{Path, PathBuf};

use crate::core::navigation::Navigation;

/// Profundidad de pre-carga: imágenes adyacentes por lado (N±1).
pub const PRELOAD_DEPTH: isize = 1;

/// Devuelve los paths a precargar, en orden de prioridad [N+1, N-1, N+2, N-2…].
///
/// # Arguments
/// * `nav` - Estado de navegación actual.
/// * `depth` - Cuántas imágenes adyacentes por lado considerar.
/// * `is_cached` - Predicado: `true` si el path ya está en cache (se excluye).
/// * `is_in_flight` - Predicado: `true` si el path ya se está cargando (se excluye).
///
/// # Returns
/// Paths a precargar, sin duplicados, sin el path actual, y sin los ya
/// cacheados ni en vuelo. Lista vacía si no hay imágenes.
pub fn preload_targets(
    nav: &Navigation,
    depth: isize,
    is_cached: impl Fn(&Path) -> bool,
    is_in_flight: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let len = nav.images.len() as isize;
    if len == 0 {
        return targets;
    }
    for d in 1..=depth {
        for offset in [d, -d] {
            let idx = (nav.current as isize + offset).rem_euclid(len) as usize;
            if idx == nav.current {
                continue;
            }
            let path = &nav.images[idx];
            if !is_cached(path) && !is_in_flight(path) && !targets.contains(path) {
                targets.push(path.clone());
            }
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav(images: &[&str], current: usize) -> Navigation {
        Navigation {
            images: images.iter().map(|s| PathBuf::from(s)).collect(),
            current,
        }
    }

    fn never(_p: &Path) -> bool {
        false
    }

    #[test]
    fn preloads_prev_and_next_in_priority_order() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 2);
        let targets = preload_targets(&n, 1, never, never);
        assert_eq!(targets, vec![PathBuf::from("3.png"), PathBuf::from("1.png")]);
    }

    #[test]
    fn wraps_circularly() {
        let n = nav(&["0.png", "1.png", "2.png"], 0);
        let targets = preload_targets(&n, 1, never, never);
        assert_eq!(targets, vec![PathBuf::from("1.png"), PathBuf::from("2.png")]);
    }

    #[test]
    fn skips_cached_paths() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 1);
        let targets = preload_targets(&n, 1, |p| p == Path::new("2.png"), never);
        assert_eq!(targets, vec![PathBuf::from("0.png")]);
    }

    #[test]
    fn skips_in_flight_paths() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 1);
        let targets = preload_targets(&n, 1, never, |p| p == Path::new("2.png"));
        assert_eq!(targets, vec![PathBuf::from("0.png")]);
    }

    #[test]
    fn depth_two_preloads_four_in_order() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png", "4.png", "5.png"], 3);
        let targets = preload_targets(&n, 2, never, never);
        assert_eq!(
            targets,
            vec![
                PathBuf::from("4.png"),
                PathBuf::from("2.png"),
                PathBuf::from("5.png"),
                PathBuf::from("1.png"),
            ]
        );
    }

    #[test]
    fn skips_current_image() {
        let n = nav(&["0.png"], 0);
        assert!(preload_targets(&n, 3, never, never).is_empty());
    }

    #[test]
    fn no_duplicates_when_depth_exceeds_size() {
        let n = nav(&["0.png", "1.png", "2.png"], 1);
        let targets = preload_targets(&n, 5, never, never);
        assert_eq!(targets.len(), 2);
    }
}
```

- [ ] **Step 4: Ejecutar los tests para verificar que pasan**

Run: `cargo test --lib core::preload`
Expected: PASS — 7 tests.

- [ ] **Step 5: Commit**

```bash
git add src/core/preload.rs src/core/mod.rs
git commit -m "feat: add pure preload_targets planner for N±1 pre-caching"
```

---

## Task 4: Integración en `app.rs` — cache + canal único + pre-carga

**Files:**
- Rewrite: `src/app.rs`

Este archivo es UI glue; no tiene unit tests directos (la lógica testeable está en
`core`). La verificación es: compila, clippy limpio, formato OK y los 75 tests de
la librería siguen pasando. La integración completa (flujos de usuario) se prueba
en S4.

- [ ] **Step 1: Reescribir `src/app.rs`**

Reemplazar `src/app.rs` completo por:

```rust
//! Estado global de la aplicación y loop principal de `egui`.

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
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{theme, toast::Toasts, viewer};
use crate::utils::errors::Result;
use crate::utils::paths::settings_path;

/// Evento enviado por un thread worker al UI thread.
///
/// La imagen decodificada NO viaja por el canal: el worker la inserta en el
/// cache y la UI la lee de ahí. El evento solo notifica el resultado del path.
struct LoadEvent {
    path: PathBuf,
    result: Result<()>,
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
    /// Cache LRU de imágenes decodificadas, compartido con los workers.
    cache: Arc<ImageCache>,
    /// Paths con una carga en curso (deduplicación de workers).
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
    /// Emisor del canal único (clonado a cada worker).
    tx: mpsc::Sender<LoadEvent>,
    /// Receptor persistente del canal único.
    rx: Option<mpsc::Receiver<LoadEvent>>,
    toasts: Toasts,
    /// `true` si el usuario ha hecho zoom/pan con la imagen actual.
    user_interacted: bool,
    /// Último tamaño del canvas; se usa para re-fitear al redimensionar.
    last_viewport: Option<Vec2>,
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
        }
    }

    /// Carga la configuración y devuelve un error tipado si falla.
    ///
    /// Expuesta para que los tests de integración puedan verificar el ciclo
    /// de vida completo sin arrancar una ventana.
    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }

    /// Guard del set de paths en carga, recuperándose de un lock envenenado.
    fn in_flight_guard(&self) -> MutexGuard<'_, HashSet<PathBuf>> {
        self.in_flight.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Abre el diálogo nativo y, si hay elección, carga la imagen.
    ///
    /// El tiempo de egui se re-lee tras el diálogo (que es bloqueante): usar
    /// el tiempo del frame en que se abrió haría que un toast emitido ahora
    /// expirara al instante si el diálogo estuvo abierto más de 3 segundos.
    fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Imágenes", SUPPORTED_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            let t = self.ctx.input(|i| i.time);
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
                self.toasts
                    .push(format!("No se pudo leer la carpeta: {e}"), t);
            }
        }
    }

    /// Carga `path` de la forma más rápida posible:
    ///
    /// 1. Cache hit → textura inmediata + pre-carga (sin thread).
    /// 2. In-flight → no-op (un worker ya lo está cargando).
    /// 3. Miss → spawn worker que decodifica, cachea y notifica.
    fn start_load(&mut self, path: PathBuf) {
        if let Some((texture, image_size)) = self.texture_from_cache(&path) {
            tracing::info!(path = %path.display(), "image loaded from cache");
            self.apply_decoded(texture, image_size);
            return;
        }
        if self.in_flight_guard().contains(&path) {
            tracing::debug!(path = %path.display(), "load already in flight");
            return;
        }
        self.spawn_load(path, false);
    }

    /// Construye la textura desde el cache si `path` está presente.
    ///
    /// Devuelve `(textura, tamaño de imagen)` con el guard del cache ya soltado
    /// (la `CacheEntryRef` se cae al final de la llamada), para que el caller
    /// pueda mutar `self` libremente después.
    fn texture_from_cache(&self, path: &std::path::Path) -> Option<(egui::TextureHandle, Vec2)> {
        let entry = self.cache.get(path)?;
        let texture = make_texture(&self.ctx, &entry);
        let size = entry.dimensions();
        Some((texture, Vec2::new(size.0 as f32, size.1 as f32)))
    }

    /// Aplica una imagen decodificada al estado: textura, transform en fit y
    /// dispara la pre-carga de N±1.
    fn apply_decoded(&mut self, texture: egui::TextureHandle, image_size: Vec2) {
        self.texture = Some(texture);
        self.transform = ViewTransform::new(image_size, Vec2::ZERO);
        self.user_interacted = false;
        self.last_viewport = None;
        self.preload_neighbors();
    }

    /// Spawnea un worker que decodifica `path`, lo inserta en el cache y envía
    /// un evento ligero por el canal único.
    ///
    /// `is_preload` solo cambia el nivel de log (DEBUG vs INFO): la lógica del
    /// worker es idéntica. El flag no genera toasts de error — eso lo decide el
    /// check `is_current` en `poll_loader`.
    fn spawn_load(&self, path: PathBuf, is_preload: bool) {
        if is_preload {
            tracing::debug!(path = %path.display(), "preloading image");
        } else {
            tracing::info!(path = %path.display(), "loading image");
        }
        self.in_flight_guard().insert(path.clone());
        let tx = self.tx.clone();
        let cache = self.cache.clone();
        let in_flight = self.in_flight.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = load_image(&path).map(|image| {
                cache.insert(path.clone(), image);
            });
            in_flight
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&path);
            if tx.send(LoadEvent { path, result }).is_err() {
                tracing::debug!("load event dropped (receiver gone)");
            }
            ctx.request_repaint();
        });
    }

    /// Drena el canal único; solo actúa sobre el path actual.
    ///
    /// Los eventos de pre-carga obsoletos (path distinto del actual) se ignoran
    /// silenciosamente: el único efecto que tenían era poblar el cache.
    fn poll_loader(&mut self, t: f64) {
        let Some(mut rx) = self.rx.take() else { return };
        while let Ok(event) = rx.try_recv() {
            let is_current = self
                .navigation
                .as_ref()
                .and_then(|n| n.current_path())
                .map(|p| p == &event.path)
                .unwrap_or(false);
            if !is_current {
                tracing::debug!(path = %event.path.display(), "ignoring non-current load result");
                continue;
            }
            match event.result {
                Ok(()) => {
                    tracing::info!(path = %event.path.display(), "image decoded");
                    if let Some((texture, image_size)) = self.texture_from_cache(&event.path) {
                        self.apply_decoded(texture, image_size);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, path = %event.path.display(), "failed to load image");
                    self.toasts.push(format!("No se pudo abrir: {e}"), t);
                }
            }
        }
        self.rx = Some(rx);
    }

    /// Dispara la pre-carga de N±1 usando `preload_targets`.
    fn preload_neighbors(&self) {
        let Some(nav) = &self.navigation else { return };
        let targets = preload_targets(
            nav,
            PRELOAD_DEPTH,
            |p| self.cache.contains(p),
            |p| self.in_flight_guard().contains(p),
        );
        for path in targets {
            self.spawn_load(path, true);
        }
    }

    /// Navega `dir` pasos (-1 prev, +1 next) y carga la nueva imagen.
    fn navigate(&mut self, dir: isize) {
        let Some(nav) = &mut self.navigation else {
            return;
        };
        if dir > 0 {
            nav.next();
        } else {
            nav.prev();
        }
        if let Some(path) = nav.current_path().cloned() {
            self.start_load(path);
        }
    }

    /// Atajos de teclado: Ctrl+O abre, ←→ navega, F re-ajusta a fit.
    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        let open = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O));
        if open {
            self.open_dialog();
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
}

/// Convierte una imagen decodificada en textura de egui.
fn make_texture(ctx: &egui::Context, image: &DynamicImage) -> egui::TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture("image", color_image, egui::TextureOptions::LINEAR)
}

impl eframe::App for ShImagesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), &self.settings.theme);
        let t = ui.input(|i| i.time);

        self.poll_loader(t);

        let mut want_open = false;
        egui::CentralPanel::default().show(ui, |ui| {
            egui::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Archivo", |ui| {
                    if ui.button("Abrir…").clicked() {
                        ui.close();
                        want_open = true;
                    }
                });
            });
            if want_open {
                self.open_dialog();
            }

            match &self.texture {
                Some(texture) => {
                    let resp = viewer::show(ui, texture, &mut self.transform);
                    if resp.zoomed || resp.panned {
                        self.user_interacted = true;
                    }
                    // Auto-fit: al cargar (viewport recién conocido) y al
                    // redimensionar mientras el usuario no haya interactuado.
                    let viewport = self.transform.viewport;
                    if !self.user_interacted && self.last_viewport != Some(viewport) {
                        self.transform.fit();
                        self.last_viewport = Some(viewport);
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

        self.handle_shortcuts(ui);
    }
}
```

- [ ] **Step 2: Verificar compilación**

Run: `cargo check`
Expected: 0 errores.

- [ ] **Step 3: Verificar lints y formato**

Run:
```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
Expected: 0 warnings, 0 diffs de formato.

- [ ] **Step 4: Ejecutar la suite completa**

Run: `cargo test`
Expected: PASS — 75 tests (61 previos + 4 de `contains` + 3 de `neighbor_paths` + 7 de `preload`).

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire LRU cache, single load channel and N±1 preload into app"
```

---

## Task 5: Verificación final según AGENTS.md

**Files:** ninguno (QA)

- [ ] **Step 1: Correr la suite completa en debug y release**

Run:
```bash
cargo check
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
cargo test --release
```
Expected: todo pasa, 0 warnings, 75 tests totales.

- [ ] **Step 2: Verificar ausencia de panics en producción**

Run: `rg -n "unwrap\(|expect\(" src/`
Expected: solo apariciones dentro de `#[cfg(test)]`. Nota: `lock()` usa
`unwrap_or_else` (recuperación de lock envenenado) e `in_flight_guard`/el worker
idem — no son `unwrap(`.

- [ ] **Step 3: Verificar que `core/` no depende de `egui`**

Run: `rg -n "egui|eframe" src/core/`
Expected: sin resultados.

- [ ] **Step 4: Commit final (solo si la verificación modificó algo)**

```bash
git add -A
git commit -m "chore: final verification pass for Fase 2 S2 cache + preload"
```

> Si el árbol está limpio tras `cargo fmt --check`, no hay commit.

---

## Self-Review

**Spec coverage:**
- `Arc<ImageCache>` en `ShImagesApp` ✓ (Task 4, campo `cache`)
- Canal único `mpsc` persistente ✓ (Task 4, `tx`/`rx` creados en `new()`)
- `LoadEvent { path, result: Result<()> }` sin imagen ✓ (Task 4)
- `start_load` con 3 ramas (hit / in-flight / miss) ✓ (Task 4)
- `in_flight: Arc<Mutex<HashSet<PathBuf>>>` con deduplicación ✓ (Task 4)
- Pre-carga N±1 tras mostrar imagen, saltando cached/in-flight ✓ (Task 4 + Task 3)
- `Navigation::neighbor_paths()` ✓ (Task 2)
- `core/preload.rs::preload_targets` pura y testeable ✓ (Task 3)
- `ImageCache::contains()` sin reordenar LRU ni contar hits ✓ (Task 1)
- `poll_loader` drena el canal, solo actúa sobre el path actual ✓ (Task 4)
- Pre-carga no tostifica errores ✓ (Task 4, check `is_current` antes del toast)
- Tests de los 4 añadidos ✓ (Tasks 1-3)
- Suite previa en verde ✓ (Task 4 Step 4, 75 tests)
- Sin `.unwrap()`/`.expect()` en producción ✓ (Task 5 Step 2)
- `core/` sin dependencias de `egui` ✓ (Task 5 Step 3)
- Docstrings en toda la API nueva ✓ (Tasks 1-3)

**Placeholder scan:** Sin "TBD"/"TODO" en el plan. Todo paso tiene código completo.

**Type consistency:**
- `ImageCache::contains(&self, &Path) -> bool` — consistente entre Task 1 (test) y Task 4 (uso en `preload_neighbors`).
- `Navigation::neighbor_paths(&self) -> [Option<&PathBuf>; 2]` — consistente entre Task 2 (test) y test.
- `preload_targets(nav, depth, is_cached, is_in_flight) -> Vec<PathBuf>` — consistente entre Task 3 (test/impl) y Task 4 (uso con `PRELOAD_DEPTH`).
- `PRELOAD_DEPTH: isize = 1` — constante usada en Task 4.
- `LoadEvent { path: PathBuf, result: Result<()> }` — consistente entre `spawn_load` (envío) y `poll_loader` (consumo).
- `texture_from_cache(&self, &Path) -> Option<(TextureHandle, Vec2)>` — usado en `start_load` y `poll_loader`; suelta el guard antes de devolver (evita el conflicto de borrows con `apply_decoded(&mut self)`).
- `spawn_load(&self, PathBuf, bool)` — llamado desde `start_load` (false) y `preload_neighbors` (true); `&self` permite el patrón de pre-carga desde métodos inmutables.
