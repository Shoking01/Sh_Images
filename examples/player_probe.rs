//! Player-layer probe: exercises the REAL VideoPlayer (threads, cpal, clock)
//! headless and prints position/buffer/ring state every 200ms for 6 seconds.
//!
//! Run: cargo run --release --features video --example player_probe -- <path>

use std::time::{Duration, Instant};

use sh_images::core::video::VideoPlayer;

fn main() {
    let path = std::env::args().nth(1).expect("path");
    std::env::set_var("SH_IMAGES_FFMPEG_REAL", "1");

    let player = std::sync::Arc::new(std::sync::Mutex::new(VideoPlayer::open(
        std::path::Path::new(&path),
        (1280, 720),
        80,
        true, // autoplay
    )));

    // Simulador de UI: llama tick() a 60 fps como haría el loop de egui.
    let ticker = std::sync::Arc::clone(&player);
    std::thread::spawn(move || {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(6) {
            if let Ok(mut p) = ticker.try_lock() {
                p.tick(Instant::now());
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    });

    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(6) {
        std::thread::sleep(Duration::from_millis(200));
        let now = Instant::now();
        let (snap, audio_len, ring_len, rate) = {
            let p = player.lock().unwrap();
            (
                p.snapshot(now),
                p.audio_len(),
                p.ring_len(),
                p.audio_sample_rate(),
            )
        };
        println!(
            "t={:.1}s pos={:.2}s state={:?} ready={} audio_q={audio_len} ring={ring_len} rate={rate} presented_err={:?}",
            t0.elapsed().as_secs_f32(),
            snap.position.as_secs_f32(),
            snap.state,
            player.lock().unwrap().is_ready(),
            snap.error,
        );
        if let Some(err) = snap.error {
            println!("PLAYER ERROR: {err}");
            break;
        }
    }
}
