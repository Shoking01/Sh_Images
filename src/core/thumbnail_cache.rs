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
    use std::path::{Path, PathBuf};

    use image::{DynamicImage, GenericImageView, RgbaImage};

    use super::*;

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
            cache
                .get(Path::new("a.png"))
                .expect("cacheada")
                .dimensions(),
            (50, 30)
        );
    }
}
