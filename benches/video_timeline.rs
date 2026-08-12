//! Presupuesto: la selección de frame debe ser O(log n), no O(n).
//!
//! `LoadedImage::frame_index_at` recorre linealmente, y es correcto para los
//! 120 frames como mucho de un GIF. Un video de 2 h a 30 fps son 216 000 marcas
//! temporales consultadas 30-60 veces por segundo: con búsqueda lineal la UI se
//! hunde. Este bench existe para que nadie "simplifique" `partition_point` de
//! vuelta a un bucle.
//!
//! Mide sólo lógica pura: sin decodificador, sin GPU, sin audio. Corre igual en
//! las tres plataformas.

use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::video::timeline::{
    frame_decision, frame_index_at, time_to_next_frame, LATE_THRESHOLD,
};

/// Marcas temporales de un video a 30 fps.
fn pts_list(count: usize) -> Vec<Duration> {
    (0..count)
        .map(|i| Duration::from_micros(i as u64 * 33_333))
        .collect()
}

fn bench_frame_index_at(c: &mut Criterion) {
    // 10 000 frames ≈ 5,5 minutos a 30 fps.
    let pts = pts_list(10_000);
    let mitad = pts[5_000];
    c.bench_function("video_frame_index_at_10k_pts", |b| {
        b.iter(|| frame_index_at(black_box(&pts), black_box(mitad)))
    });

    // 216 000 frames ≈ 2 horas. Con búsqueda binaria apenas debe notarse la
    // diferencia respecto al caso de 10 000.
    let largo = pts_list(216_000);
    let final_ = largo[215_999];
    c.bench_function("video_frame_index_at_two_hours", |b| {
        b.iter(|| frame_index_at(black_box(&largo), black_box(final_)))
    });
}

fn bench_frame_decision(c: &mut Criterion) {
    // Se ejecuta en el hilo de UI 30-60 veces por segundo: detecta que alguien
    // meta una asignación o un `format!` en el camino caliente.
    let pts = Duration::from_millis(1_000);
    let clock = Duration::from_millis(1_005);
    c.bench_function("video_frame_decision", |b| {
        b.iter(|| frame_decision(black_box(pts), black_box(clock), black_box(LATE_THRESHOLD)))
    });

    c.bench_function("video_time_to_next_frame", |b| {
        b.iter(|| {
            time_to_next_frame(
                black_box(Some(Duration::from_millis(1_033))),
                black_box(clock),
                black_box(Some(30.0)),
            )
        })
    });
}

criterion_group!(benches, bench_frame_index_at, bench_frame_decision);
criterion_main!(benches);
