use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use image::DynamicImage;

#[derive(Default)]
struct ThumbCacheInner {
    map: HashMap<PathBuf, DynamicImage>,
}
#[derive(Default)]
pub struct ThumbnailCache {
    inner: Mutex<ThumbCacheInner>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, ThumbCacheInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub fn insert(&self, path: PathBuf, image: DynamicImage) {
        self.lock().map.insert(path, image);
    }
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
    pub fn contains(&self, path: &Path) -> bool {
        self.lock().map.contains_key(path)
    }
    pub fn clear(&self) {
        self.lock().map.clear();
    }
    pub fn len(&self) -> usize {
        self.lock().map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
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
