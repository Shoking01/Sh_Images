//! Benchmark: generación de miniatura 96px desde una imagen 4K.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::thumbnail_gen::{generate_thumbnail, THUMB_MAX};

mod common;

use common::{gradient_image, RES_4K};

fn bench_thumbnail(c: &mut Criterion) {
    let image = gradient_image(RES_4K.0, RES_4K.1);
    c.bench_function("thumbnail_4k_to_96", |b| {
        b.iter(|| {
            let thumb = generate_thumbnail(black_box(&image), THUMB_MAX);
            black_box(thumb);
        })
    });
}

criterion_group!(benches, bench_thumbnail);
criterion_main!(benches);
