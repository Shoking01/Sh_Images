//! Throughput harness: decode the user's problem file as fast as possible and
//! report wall-clock fps capacity, split video/audio, plus position trace.
//!
//! Run: cargo run --release --features video --example decode_throughput -- <path>

use std::time::Instant;

use sh_images::core::video::backend::Sample;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2026-08-24 13-57-02.mp4".to_string());
    std::env::set_var("SH_IMAGES_FFMPEG_REAL", "1");

    let mut dec =
        sh_images::core::video::backend::open(std::path::Path::new(&path), (1280, 720), 48_000)
            .expect("open");
    let info = dec.info();
    println!(
        "open ok: {}x{} fps={:?} dur={:?} has_audio={} hw={}",
        info.width, info.height, info.fps, info.duration, info.has_audio, info.hw_active
    );

    let t0 = Instant::now();
    let (mut nv, mut na, mut max_pts, mut last_video_pts, mut errs) =
        (0u64, 0u64, f64::MIN, f64::MIN, 0u64);
    let mut slowest = 0f64;
    let mut last = t0;
    loop {
        match dec.next_sample() {
            Ok(Sample::Video(f)) => {
                nv += 1;
                let pts = f.pts.as_secs_f64();
                if pts > max_pts {
                    max_pts = pts;
                }
                let now = last.elapsed().as_secs_f64();
                if now > slowest {
                    slowest = now;
                }
                if pts < last_video_pts - 0.5 {
                    println!("PTS REGRESSION: {last_video_pts:.3} -> {pts:.3}");
                }
                last_video_pts = pts;
                last = Instant::now();
            }
            Ok(Sample::Audio { pts, samples }) => {
                na += 1;
                let _ = samples.len();
                let pts = pts.as_secs_f64();
                if pts > max_pts {
                    max_pts = pts;
                }
            }
            Ok(Sample::EndOfStream) => {
                println!("EOS at video_pts={last_video_pts:.3}");
                break;
            }
            Err(e) => {
                errs += 1;
                println!("ERR: {e}");
                if errs > 3 {
                    break;
                }
            }
        }
    }
    let dt = t0.elapsed().as_secs_f64();
    let slowest_ms = slowest * 1000.0;
    println!(
        "decoded {nv} video + {na} audio samples in {dt:.2}s => capacity {:.0} video fps (slowest single call {slowest_ms:.1} ms), max pts {max_pts:.2}s, errors {errs}",
        nv as f64 / dt
    );
}
