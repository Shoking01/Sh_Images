use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use image::DynamicImage;

const NO_NODE: usize = usize::MAX;
const DEFAULT_LIMIT_MB: u64 = 64;

struct Node {
    key: PathBuf,
    image: DynamicImage,
    bytes: u64,
    prev: usize,
    next: usize,
}

#[derive(Default)]
struct ThumbCacheInner {
    map: HashMap<PathBuf, usize>,
    nodes: Vec<Node>,
    head: usize,
    tail: usize,
    memory_used: u64,
    memory_limit_mb: u64,
}

impl ThumbCacheInner {
    fn limit_bytes(&self) -> u64 {
        self.memory_limit_mb.saturating_mul(1024 * 1024)
    }
    fn push_front(&mut self, index: usize) {
        let old_head = self.head;
        self.nodes[index].prev = NO_NODE;
        self.nodes[index].next = old_head;
        if old_head != NO_NODE {
            self.nodes[old_head].prev = index;
        } else {
            self.tail = index;
        }
        self.head = index;
    }
    fn unlink(&mut self, index: usize) {
        let (prev, next) = (self.nodes[index].prev, self.nodes[index].next);
        if prev != NO_NODE {
            self.nodes[prev].next = next;
        } else {
            self.head = next;
        }
        if next != NO_NODE {
            self.nodes[next].prev = prev;
        } else {
            self.tail = prev;
        }
        self.nodes[index].prev = NO_NODE;
        self.nodes[index].next = NO_NODE;
    }
    fn move_to_front(&mut self, index: usize) {
        if self.head == index {
            return;
        }
        self.unlink(index);
        self.push_front(index);
    }
    fn remove_node(&mut self, index: usize) -> Node {
        let last = self.nodes.len() - 1;
        let node = self.nodes.swap_remove(index);
        if index != last {
            let moved_key = self.nodes[index].key.clone();
            self.map.insert(moved_key, index);
            let (mprev, mnext) = (self.nodes[index].prev, self.nodes[index].next);
            if mprev != NO_NODE {
                self.nodes[mprev].next = index;
            } else {
                self.head = index;
            }
            if mnext != NO_NODE {
                self.nodes[mnext].prev = index;
            } else {
                self.tail = index;
            }
        }
        node
    }
    fn evict_to_limit(&mut self) {
        while self.memory_used > self.limit_bytes() {
            if self.tail == NO_NODE {
                break;
            }
            let tail = self.tail;
            let key = self.nodes[tail].key.clone();
            self.unlink(tail);
            let node = self.remove_node(tail);
            self.memory_used = self.memory_used.saturating_sub(node.bytes);
            self.map.remove(&key);
        }
    }
}

#[derive(Default)]
pub struct ThumbnailCache {
    inner: Mutex<ThumbCacheInner>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_LIMIT_MB)
    }
    pub fn with_limit(memory_limit_mb: u64) -> Self {
        Self {
            inner: Mutex::new(ThumbCacheInner {
                map: HashMap::new(),
                nodes: Vec::new(),
                head: NO_NODE,
                tail: NO_NODE,
                memory_used: 0,
                memory_limit_mb,
            }),
        }
    }
    fn lock(&self) -> MutexGuard<'_, ThumbCacheInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub fn insert(&self, path: PathBuf, image: DynamicImage) {
        let bytes = estimate_bytes(&image);
        if bytes > self.lock().limit_bytes() {
            return;
        }
        let mut inner = self.lock();
        if let Some(&index) = inner.map.get(&path) {
            inner.memory_used = inner
                .memory_used
                .saturating_sub(inner.nodes[index].bytes)
                .saturating_add(bytes);
            inner.nodes[index].image = image;
            inner.nodes[index].bytes = bytes;
            inner.move_to_front(index);
            inner.evict_to_limit();
            return;
        }
        let index = inner.nodes.len();
        inner.nodes.push(Node {
            key: path.clone(),
            image,
            bytes,
            prev: NO_NODE,
            next: NO_NODE,
        });
        inner.map.insert(path, index);
        inner.memory_used = inner.memory_used.saturating_add(bytes);
        inner.push_front(index);
        inner.evict_to_limit();
    }
    pub fn get(&self, path: &Path) -> Option<ThumbnailRef<'_>> {
        let mut inner = self.lock();
        let &index = inner.map.get(path)?;
        inner.move_to_front(index);
        Some(ThumbnailRef {
            guard: inner,
            path: path.to_path_buf(),
        })
    }
    pub fn contains(&self, path: &Path) -> bool {
        self.lock().map.contains_key(path)
    }
    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.map.clear();
        inner.nodes.clear();
        inner.head = NO_NODE;
        inner.tail = NO_NODE;
        inner.memory_used = 0;
    }
    pub fn len(&self) -> usize {
        self.lock().map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn memory_used(&self) -> u64 {
        self.lock().memory_used
    }
    pub fn memory_limit_mb(&self) -> u64 {
        self.lock().memory_limit_mb
    }
}
pub struct ThumbnailRef<'a> {
    guard: MutexGuard<'a, ThumbCacheInner>,
    path: PathBuf,
}

impl Deref for ThumbnailRef<'_> {
    type Target = DynamicImage;
    fn deref(&self) -> &DynamicImage {
        let index = self.guard.map[&self.path];
        &self.guard.nodes[index].image
    }
}

fn estimate_bytes(image: &DynamicImage) -> u64 {
    let (w, h) = (image.width() as u64, image.height() as u64);
    let bpp = image.color().bytes_per_pixel() as u64;
    w.saturating_mul(h).saturating_mul(bpp)
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

    #[test]
    fn eviction_respects_byte_limit() {
        let cache = ThumbnailCache::with_limit(1);
        for i in 0..10 {
            cache.insert(PathBuf::from(format!("img_{i}.png")), rgba(200, 200));
        }
        assert!(cache.memory_used() <= 1024 * 1024);
        assert!(cache.len() < 10);
    }

    #[test]
    fn oversized_thumbnail_is_not_cached() {
        let cache = ThumbnailCache::with_limit(1);
        let big = rgba(2000, 2000);
        cache.insert(PathBuf::from("big.png"), big);
        assert!(!cache.contains(Path::new("big.png")));
        assert!(cache.is_empty());
    }

    #[test]
    fn get_moves_to_front() {
        let cache = ThumbnailCache::with_limit(1);
        cache.insert(PathBuf::from("a.png"), rgba(100, 100));
        cache.insert(PathBuf::from("b.png"), rgba(100, 100));
        cache.insert(PathBuf::from("c.png"), rgba(100, 100));
        assert!(cache.get(Path::new("a.png")).is_some());
        cache.insert(PathBuf::from("d.png"), rgba(100, 100));
        assert!(cache.get(Path::new("a.png")).is_some());
    }
}
