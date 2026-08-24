//! Seek probe: decode 1s, seek to target, report first video pts after seek,
//! time until pts >= target, and whether audio resumes past target.
//!
//! Run: cargo run --release --features video --example seek_probe -- <path> <target_secs>

use std::time::Instant;

use sh_images::core::video::backend::Sample;

fn main() {
    let path = std::env::args().nth(1).expect("path");
    let target = std::time::Duration::from_secs_f64(
        std::env::args()
            .nth(2)
            .expect("target_secs")
            .parse()
            .unwrap(),
    );
    std::env::set_var("SH_IMAGES_FFMPEG_REAL", "1");

    let mut dec =
        sh_images::core::video::backend::open(std::path::Path::new(&path), (1280, 720), 48_000)
            .expect("open");

    // Decode ~1s worth of video first (60 frames @60fps).
    let mut seen = 0u64;
    let mut audio_after_seek = false;
    while seen < 60 {
        match dec.next_sample().expect("decode") {
            Sample::Video(_) => seen += 1,
            Sample::Audio { .. } => {}
            Sample::EndOfStream => panic!("EOS before warmup finished"),
        }
    }
    println!("warmup: 60 video frames decoded");

    let t0 = Instant::now();
    dec.seek(target).expect("seek must succeed");
    println!("seek({:?}) issued in {:?}", target, t0.elapsed());

    let mut first_video_pts: Option<f64> = None;
    let mut reached: Option<std::time::Duration> = None;
    let mut first_audio_pts: Option<f64> = None;
    let t1 = Instant::now();
    for i in 0..2000 {
        match dec.next_sample().expect("decode after seek") {
            Sample::Video(f) => {
                let pts = f.pts.as_secs_f64();
                if i < 3 {
                    println!("video pts after seek: {pts:.3}");
                }
                if first_video_pts.is_none() {
                    first_video_pts = Some(pts);
                }
                if pts >= target.as_secs_f64() && reached.is_none() {
                    reached = Some(t1.elapsed());
                }
                if pts >= target.as_secs_f64() + 1.0 {
                    break;
                }
            }
            Sample::Audio { pts, .. } => {
                let p = pts.as_secs_f64();
                if first_audio_pts.is_none() {
                    first_audio_pts = Some(p);
                    println!(
                        "first audio pts after seek: {p:.3} (target {:.3})",
                        target.as_secs_f64()
                    );
                }
                if p >= target.as_secs_f64() {
                    audio_after_seek = true;
                }
            }
            Sample::EndOfStream => {
                println!("EOS");
                break;
            }
        }
    }
    match (first_video_pts, reached) {
        (Some(fv), Some(r)) => println!(
            "SEEK OK: first video pts {fv:.3}, reached target after {:.2?}, audio past target: {audio_after_seek}",
            r
        ),
        (Some(fv), None) => println!(
            "SEEK STALLED: first video pts {fv:.3} but NEVER reached target {:.3}",
            target.as_secs_f64()
        ),
        (None, _) => println!("SEEK BROKEN: no video frames after seek"),
    }
}
