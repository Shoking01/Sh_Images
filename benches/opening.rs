//! Benchmark base: tiempo de apertura/decodificación de una imagen.

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::image_loader::load_image;

/// Abre el fixture PNG y verifica que decodifica correctamente.
fn open_fixture() -> bool {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample.png");
    load_image(&path).is_ok()
}

fn bench_opening(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample.png");

    c.bench_function("open_png_fixture", |b| {
        assert!(open_fixture(), "fixture sample.png debe decodificar");
        b.iter(|| {
            let ok = load_image(black_box(&path)).is_ok();
            black_box(ok);
        })
    });
}

criterion_group!(benches, bench_opening);
criterion_main!(benches);
