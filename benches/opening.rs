//! Benchmark: tiempo de apertura/decodificación de imágenes a varias
//! resoluciones (AGENTS.md §6.2: 1080p < 100ms, 4K < 200ms, 8K < 500ms).

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use image::ImageFormat;
use sh_images::core::image_loader::load_image;
use tempfile::tempdir;

mod common;

use common::{synthetic_image_path, RES_1080P, RES_4K, RES_8K};

/// Crea el directorio temp con una imagen sintética de la resolución dada.
fn setup_synthetic(format: ImageFormat, (w, h): (u32, u32)) -> (tempfile::TempDir, Box<Path>) {
    let dir = tempdir().expect("tempdir para benchmark");
    let path = synthetic_image_path(dir.path(), w, h, format);
    (dir, path.into_boxed_path())
}

/// Mide `load_image` para una imagen ya generada en disco.
fn bench_open(c: &mut Criterion, name: &str, dir: &tempfile::TempDir, path: &Path) {
    c.bench_function(name, |b| {
        b.iter(|| {
            let ok = load_image(black_box(path)).is_ok();
            black_box(ok);
        })
    });
    // `dir` se mantiene vivo hasta el final del grupo para que el archivo exista.
    let _ = dir;
}

fn bench_opening(c: &mut Criterion) {
    for format in [ImageFormat::Png, ImageFormat::Jpeg] {
        let ext = format.extensions_str()[0];
        for (label, res) in [("1080p", RES_1080P), ("4k", RES_4K), ("8k", RES_8K)] {
            let (dir, path) = setup_synthetic(format, res);
            bench_open(c, &format!("open_{ext}_{label}"), &dir, &path);
        }
    }
}

criterion_group!(benches, bench_opening);
criterion_main!(benches);
