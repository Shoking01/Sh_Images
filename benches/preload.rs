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
