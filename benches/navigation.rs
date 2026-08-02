//! Benchmark: navegación entre imágenes de una carpeta (AGENTS.md §6.2:
//! latencia de navegación < 50ms).

use std::fs;
use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use tempfile::tempdir;

/// Crea `n` archivos `.jpg` (vacíos) en una carpeta temp y devuelve (dir, paths).
fn setup_folder(n: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempdir().expect("tempdir para benchmark");
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let path = dir.path().join(format!("img_{i:04}.jpg"));
        fs::File::create(&path).expect("crear archivo de imagen tocado");
        paths.push(path);
    }
    (dir, paths)
}

fn bench_navigation(c: &mut Criterion) {
    let (_dir, paths) = setup_folder(1000);
    let anchor = &paths[0];

    c.bench_function("navigation_from_folder_1000", |b| {
        b.iter(|| {
            let nav = Navigation::from_folder(black_box(anchor), SUPPORTED_EXTENSIONS)
                .expect("from_folder debe funcionar");
            black_box(nav.images.len());
        })
    });

    let mut nav =
        Navigation::from_folder(anchor, SUPPORTED_EXTENSIONS).expect("from_folder inicial");

    c.bench_function("navigation_next_1000", |b| {
        b.iter(|| {
            nav.next();
        })
    });

    c.bench_function("navigation_prev_1000", |b| {
        b.iter(|| {
            nav.prev();
        })
    });

    c.bench_function("navigation_neighbor_paths_1000", |b| {
        b.iter(|| {
            black_box(nav.neighbor_paths());
        })
    });
}

criterion_group!(benches, bench_navigation);
criterion_main!(benches);
