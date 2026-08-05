use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use image::DynamicImage;

use crate::core::image_loader::LoadedImage;
pub const DEFAULT_MEMORY_LIMIT_MB: u64 = 512;
const NO_NODE: usize = usize::MAX;
struct CacheEntry {
    image: LoadedImage,
    bytes: u64,
}
struct Node {
    key: PathBuf,
    value: CacheEntry,
    prev: usize,
    next: usize,
}
struct CacheInner {
    map: HashMap<PathBuf, usize>,
    nodes: Vec<Node>,
    head: usize,
    tail: usize,
    memory_used: u64,
    memory_limit_mb: u64,
    hit_count: u64,
    miss_count: u64,
}

impl CacheInner {
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
    fn clear(&mut self) {
        self.map.clear();
        self.nodes.clear();
        self.head = NO_NODE;
        self.tail = NO_NODE;
        self.memory_used = 0;
    }
    fn insert(&mut self, path: PathBuf, image: LoadedImage) -> InsertResult {
        if matches!(&image, LoadedImage::Animated(anim) if anim.frames.is_empty()) {
            return InsertResult {
                cached: false,
                evicted_keys: Vec::new(),
            };
        }

        let bytes = estimate_loaded_bytes(&image);

        if bytes > self.limit_bytes() {
            return InsertResult {
                cached: false,
                evicted_keys: Vec::new(),
            };
        }
        if let Some(&index) = self.map.get(&path) {
            let old_bytes = self.nodes[index].value.bytes;
            self.memory_used = self
                .memory_used
                .saturating_sub(old_bytes)
                .saturating_add(bytes);
            self.nodes[index].value = CacheEntry { image, bytes };
            self.move_to_front(index);
            let evicted_keys = self.evict_to_limit();
            return InsertResult {
                cached: true,
                evicted_keys,
            };
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            key: path.clone(),
            value: CacheEntry { image, bytes },
            prev: NO_NODE,
            next: NO_NODE,
        });
        self.map.insert(path, index);
        self.memory_used = self.memory_used.saturating_add(bytes);
        self.push_front(index);
        let evicted_keys = self.evict_to_limit();

        InsertResult {
            cached: true,
            evicted_keys,
        }
    }
    fn evict_to_limit(&mut self) -> Vec<PathBuf> {
        let mut evicted = Vec::new();
        while self.memory_used > self.limit_bytes() {
            if self.tail == NO_NODE {
                tracing::warn!("lru cache invariant violated; clearing cache");
                self.clear();
                evicted.clear();
                return evicted;
            }
            let tail = self.tail;
            let key = self.nodes[tail].key.clone();
            self.unlink(tail);
            let node = self.remove_node(tail);
            self.memory_used = self.memory_used.saturating_sub(node.value.bytes);
            self.map.remove(&key);
            evicted.push(key);
        }
        evicted
    }
    fn get_index(&mut self, path: &Path) -> Option<usize> {
        match self.map.get(path).copied() {
            Some(index) => {
                self.hit_count = self.hit_count.saturating_add(1);
                self.move_to_front(index);
                Some(index)
            }
            None => {
                self.miss_count = self.miss_count.saturating_add(1);
                None
            }
        }
    }
}
pub struct ImageCache {
    inner: Mutex<CacheInner>,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new(DEFAULT_MEMORY_LIMIT_MB)
    }
}

impl ImageCache {
    pub fn new(memory_limit_mb: u64) -> Self {
        Self {
            inner: Mutex::new(CacheInner {
                map: HashMap::new(),
                nodes: Vec::new(),
                head: NO_NODE,
                tail: NO_NODE,
                memory_used: 0,
                memory_limit_mb,
                hit_count: 0,
                miss_count: 0,
            }),
        }
    }
    fn lock(&self) -> MutexGuard<'_, CacheInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub fn insert_loaded(&self, path: PathBuf, image: LoadedImage) -> InsertResult {
        self.lock().insert(path, image)
    }
    pub fn insert(&self, path: PathBuf, image: DynamicImage) -> InsertResult {
        self.insert_loaded(path, LoadedImage::Static(image))
    }
    pub fn get(&self, path: &Path) -> Option<CacheEntryRef<'_>> {
        let mut inner = self.lock();
        let index = inner.get_index(path)?;
        Some(CacheEntryRef {
            guard: inner,
            index,
        })
    }
    pub fn contains(&self, path: &Path) -> bool {
        self.lock().map.contains_key(path)
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
    pub fn hit_ratio(&self) -> f32 {
        let inner = self.lock();
        let total = inner.hit_count + inner.miss_count;
        if total == 0 {
            0.0
        } else {
            inner.hit_count as f32 / total as f32
        }
    }
}
pub struct InsertResult {
    pub cached: bool,
    pub evicted_keys: Vec<PathBuf>,
}
pub struct CacheEntryRef<'a> {
    guard: MutexGuard<'a, CacheInner>,
    index: usize,
}

impl Deref for CacheEntryRef<'_> {
    type Target = LoadedImage;
    fn deref(&self) -> &LoadedImage {
        &self.guard.nodes[self.index].value.image
    }
}
fn estimate_bytes(image: &DynamicImage) -> u64 {
    let (w, h) = (image.width() as u64, image.height() as u64);
    let bpp = image.color().bytes_per_pixel() as u64;
    w.saturating_mul(h).saturating_mul(bpp)
}
fn estimate_loaded_bytes(image: &LoadedImage) -> u64 {
    match image {
        LoadedImage::Static(img) => estimate_bytes(img),
        LoadedImage::Animated(anim) => anim.frames.iter().map(|f| estimate_bytes(&f.image)).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;
    use std::time::Duration;

    use crate::core::image_loader::{AnimatedFrame, AnimatedImage, LoadedImage};

    const MIB: u64 = 1024 * 1024;
    fn rgba(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn estimate_bytes_counts_channels() {
        assert_eq!(estimate_bytes(&rgba(16, 16)), 16 * 16 * 4);
        let rgb = DynamicImage::ImageRgb8(image::RgbImage::new(16, 16));
        assert_eq!(estimate_bytes(&rgb), 16 * 16 * 3);
    }

    #[test]
    fn estimate_bytes_counts_high_depth_and_float_channels() {
        let rgb16 = DynamicImage::ImageRgb16(image::ImageBuffer::new(4, 4));
        let rgba16 = DynamicImage::ImageRgba16(image::ImageBuffer::new(4, 4));
        assert_eq!(estimate_bytes(&rgb16), 4 * 4 * 6);
        assert_eq!(estimate_bytes(&rgba16), 4 * 4 * 8);
    }

    #[test]
    fn default_matches_settings_default() {
        let cache = ImageCache::default();
        assert_eq!(cache.memory_limit_mb(), 512);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn new_with_limit_exposes_limit() {
        let cache = ImageCache::new(64);
        assert_eq!(cache.memory_limit_mb(), 64);
    }

    #[test]
    fn is_empty_reflects_state() {
        let cache = ImageCache::new(1);
        assert!(cache.is_empty());
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn insert_then_get_roundtrips_small_image() {
        let cache = ImageCache::new(1);
        let res = cache.insert(PathBuf::from("a.png"), rgba(64, 32));
        assert!(res.cached);
        assert!(res.evicted_keys.is_empty());
        let got = cache
            .get(Path::new("a.png"))
            .expect("debería estar cacheada");
        assert_eq!(got.dimensions(), (64, 32));
    }

    #[test]
    fn get_on_missing_path_returns_none() {
        let cache = ImageCache::new(1);
        assert!(cache.get(Path::new("nope.png")).is_none());
    }

    #[test]
    fn insert_existing_path_replaces_entry() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(256, 256));
        let res = cache.insert(PathBuf::from("a.png"), rgba(128, 128));
        assert!(res.cached);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.memory_used(), 128 * 128 * 4);
        assert_eq!(
            cache
                .get(Path::new("a.png"))
                .expect("cacheada")
                .dimensions(),
            (128, 128)
        );
    }

    #[test]
    fn oversized_image_is_not_cached_all_or_nothing() {
        let cache = ImageCache::new(1); // 1 MiB
        cache.insert(PathBuf::from("a.png"), rgba(256, 256)); // 256 KiB
        let res = cache.insert(PathBuf::from("big.png"), rgba(1024, 1024)); // 4 MiB
        assert!(!res.cached);
        assert!(res.evicted_keys.is_empty());
        assert_eq!(cache.len(), 1);
        assert!(cache.get(Path::new("a.png")).is_some());
        assert!(cache.get(Path::new("big.png")).is_none());
    }

    #[test]
    fn zero_dimension_image_fits() {
        let cache = ImageCache::new(1);
        let res = cache.insert(PathBuf::from("zero.png"), rgba(0, 64));
        assert!(res.cached);
        assert_eq!(cache.memory_used(), 0);
    }

    #[test]
    fn zero_memory_limit_rejects_normal_image() {
        let cache = ImageCache::new(0);
        let res = cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(!res.cached);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn eviction_removes_least_recently_used_first() {
        let cache = ImageCache::new(1); // 1 MiB = 4 × 256 KiB
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.memory_used(), 4 * 256 * 256 * 4);

        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        assert!(res.cached);
        assert_eq!(res.evicted_keys, vec![PathBuf::from("a.png")]);
        assert_eq!(cache.len(), 4);
        assert!(cache.get(Path::new("a.png")).is_none());
        for name in ["b.png", "c.png", "d.png", "e.png"] {
            assert!(cache.get(Path::new(name)).is_some());
        }
    }

    #[test]
    fn get_moves_entry_to_most_recent() {
        let cache = ImageCache::new(1);
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        assert!(cache.get(Path::new("a.png")).is_some());
        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        assert_eq!(res.evicted_keys, vec![PathBuf::from("b.png")]);
        assert!(cache.get(Path::new("a.png")).is_some());
        assert!(cache.get(Path::new("b.png")).is_none());
    }

    #[test]
    fn memory_used_never_exceeds_limit() {
        let cache = ImageCache::new(1);
        for i in 0..50 {
            let name = format!("img_{i}.png");
            cache.insert(PathBuf::from(&name), rgba(64, 64)); // 16 KiB cada una
            assert!(cache.memory_used() <= MIB);
        }
    }

    #[test]
    fn replace_that_exceeds_limit_evicts_lru_entries() {
        let cache = ImageCache::new(1);
        for i in 0..100 {
            let name = format!("img_{i:03}.png");
            cache.insert(PathBuf::from(&name), rgba(32, 80)); // 32*80*4 = 10240 B
        }
        assert_eq!(cache.memory_used(), 100 * 32 * 80 * 4);
        assert_eq!(cache.len(), 100);
        let res = cache.insert(PathBuf::from("img_000.png"), rgba(480, 480)); // 921600 B
        assert!(res.cached);
        assert!(
            !res.evicted_keys.is_empty(),
            "reemplazo grande debe evictar"
        );
        assert!(
            cache.memory_used() <= MIB,
            "memory_used debe quedar bajo el límite"
        );
        assert!(cache.get(Path::new("img_000.png")).is_some());
    }

    #[test]
    fn memory_used_and_len_correct_after_evictions() {
        let cache = ImageCache::new(1);
        for name in ["a.png", "b.png", "c.png", "d.png", "e.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.memory_used(), 4 * 256 * 256 * 4);
        for name in ["b.png", "c.png", "d.png", "e.png"] {
            assert!(cache.get(Path::new(name)).is_some());
        }
        assert!(cache.get(Path::new("a.png")).is_none());
    }

    #[test]
    fn hit_ratio_tracks_hits_and_misses() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert_eq!(cache.hit_ratio(), 0.0);

        assert!(cache.get(Path::new("a.png")).is_some()); // hit
        assert!(cache.get(Path::new("a.png")).is_some()); // hit
        assert!(cache.get(Path::new("b.png")).is_none()); // miss
        let ratio = cache.hit_ratio();
        assert!((ratio - 2.0 / 3.0).abs() < 1e-3);
    }

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
        assert!(cache.contains(Path::new("a.png")));
        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
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
    fn inserting_empty_animated_image_is_not_cached() {
        let cache = ImageCache::new(4);
        let empty = LoadedImage::Animated(AnimatedImage {
            frames: Vec::new(),
            total_duration: Duration::ZERO,
        });

        let res = cache.insert_loaded(PathBuf::from("empty.gif"), empty);
        assert!(!res.cached);
        assert!(res.evicted_keys.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.memory_used(), 0);
        assert!(cache.get(Path::new("empty.gif")).is_none());
        assert!(!cache.contains(Path::new("empty.gif")));
    }

    #[test]
    fn animated_lru_eviction_respects_summed_size() {
        let cache = ImageCache::new(1);
        let res = cache.insert_loaded(PathBuf::from("a.gif"), animated_big());
        assert!(res.cached);
        let res2 = cache.insert_loaded(PathBuf::from("b.gif"), animated_big());
        assert!(res2.cached);
        assert_eq!(cache.len(), 2);
        let res3 = cache.insert_loaded(PathBuf::from("c.gif"), animated_big());
        assert!(res3.cached);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(Path::new("a.gif")).is_none());
        assert!(cache.get(Path::new("b.gif")).is_some());
        assert!(cache.get(Path::new("c.gif")).is_some());
    }

    fn animated_big() -> LoadedImage {
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
}
