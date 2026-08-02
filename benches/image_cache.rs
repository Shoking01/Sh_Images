//! Benchmark: operaciones del LRU cache sobre una imagen 4K decodificada.

use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::image_cache::ImageCache;

mod common;

use common::RES_4K;

fn bench_cache(c: &mut Criterion) {
    let cache = ImageCache::new(512);

    // Imagen 4K "fresca" construida por iteración. NO clonar una preexistente:
    // la copia profunda de una RgbaImage 4K (~15 ms) dominaría la medición y
    // ocultaría el coste real del LRU (AGENTS.md §6.3).
    let new_image = || image::DynamicImage::ImageRgba8(image::RgbaImage::new(RES_4K.0, RES_4K.1));

    c.bench_function("cache_insert_4k", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            let path = PathBuf::from(format!("/bench/img_{i}.png"));
            let res = cache.insert(black_box(path), new_image());
            black_box(res.cached);
        })
    });

    // Cache poblado (~16 × 33 MiB ≈ llena 512 MiB) para medir get/contains
    // sobre un cache realista; `/bench/known.png` es la entrada más LRU.
    let known = PathBuf::from("/bench/known.png");
    cache.insert(known.clone(), new_image());
    for k in 0..15u32 {
        let filler = PathBuf::from(format!("/bench/fill_{k}.png"));
        cache.insert(filler, new_image());
    }

    c.bench_function("cache_get_4k", |b| {
        b.iter(|| {
            let entry = cache.get(black_box(&known));
            black_box(entry.is_some());
        })
    });

    c.bench_function("cache_contains_4k", |b| {
        b.iter(|| {
            black_box(cache.contains(black_box(&known)));
        })
    });
}

criterion_group!(benches, bench_cache);
criterion_main!(benches);
