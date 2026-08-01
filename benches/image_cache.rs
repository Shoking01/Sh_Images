//! Benchmark: operaciones del LRU cache sobre una imagen 4K decodificada.

use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::image_cache::ImageCache;

mod common;

use common::{gradient_image, RES_4K};

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
