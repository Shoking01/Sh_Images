//! Cache LRU de imágenes decodificadas, thread-safe.
//!
//! `core/` no depende de `egui` (AGENTS.md §3.2). Implementación completa en el
//! siguiente task: aquí solo viven los tests (TDD, fail-first).

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    const MIB: u64 = 1024 * 1024;

    /// Helper: imagen RGBA de `w x h` (4 B/px).
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
        let got = cache.get(Path::new("a.png")).expect("debería estar cacheada");
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
            cache.get(Path::new("a.png")).expect("cacheada").dimensions(),
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
        // Acceder a la más vieja (a) la mueve al frente (MRU).
        assert!(cache.get(Path::new("a.png")).is_some());
        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        // La evictada ahora es b, no a.
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
            assert!(cache.memory_used() <= 1 * MIB);
        }
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
}
