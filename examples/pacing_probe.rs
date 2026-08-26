//! Pacing probe: simulates the egui repaint loop faithfully — tick() then
//! sleep exactly `repaint_after()` — and histograms the intervals between
//! PRESENTED frames. A healthy 60fps source should cluster ~16.7ms; a
//! speed-changer bug shows a bimodal 16/33ms pattern.
//!
//! Run: cargo run --release --features video --example pacing_probe -- <path>

use std::time::{Duration, Instant};

use sh_images::core::video::VideoPlayer;

fn main() {
    let path = std::env::args().nth(1).expect("path");
    let secs: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    std::env::set_var("SH_IMAGES_FFMPEG_REAL", "1");

    let mut player = VideoPlayer::open(std::path::Path::new(&path), (1280, 720), 80, true);
    // Esperar a que el decodificador publique info (como hace la UI).
    while !player.is_ready() {
        std::thread::sleep(Duration::from_millis(5));
    }

    let mut last_present: Option<Instant> = None;
    let (mut under20, mut m20_40, mut over40, mut total) = (0u32, 0u32, 0u32, 0u32);
    let (_waits, _presents, _drops) = (0u32, 0u32, 0u32);
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs(secs);

    // Pausa breve post-open para replicar el arranque de la UI.
    std::thread::sleep(Duration::from_millis(50));
    player.dispatch(sh_images::core::video::PlaybackEvent::Play, Instant::now());

    while Instant::now() < deadline {
        let now = Instant::now();
        let _frame = player.tick(now);
        // Clasificar lo que decidió este tick mirando el ring/clock no basta:
        // tick devuelve sólo el presentado. Contamos presentados y su cadencia.
        if _frame.is_some() {
            total += 1;
            if let Some(l) = last_present {
                let dt = l.elapsed().as_millis();
                if dt < 20 {
                    under20 += 1;
                } else if dt <= 40 {
                    m20_40 += 1;
                } else {
                    over40 += 1;
                }
            }
            last_present = Some(Instant::now());
        }
        let wait = player
            .repaint_after(now)
            .unwrap_or(Duration::from_millis(16));
        let clock_pos = player.snapshot(Instant::now()).position;
        if total % 20 == 0 {
            println!(
                "[probe] t={:.2} presentados={total} wait={wait:?} pos={:.3} ring={}",
                t0.elapsed().as_secs_f32(),
                clock_pos.as_secs_f32(),
                player.ring_len(),
            );
        }
        // Espera de alta precisión: sleep grueso + spin fino. El sleep de
        // Windows redondea a múltiplos de ~15.6 ms y enmascararía el pacing
        // real del player con ruido del scheduler.
        let wake = Instant::now() + wait;
        while Instant::now() < wake {
            std::thread::sleep(Duration::from_micros(500));
            std::hint::spin_loop();
        }
    }

    println!("presentados={total} intervalos: <20ms={under20}  20-40ms={m20_40}  >40ms={over40}");
    if m20_40 + over40 > under20 / 4 {
        println!("PATRÓN BIMODAL — speed-changer confirmado a nivel pacing");
    } else {
        println!("pacing monótono — el tironazo no está en la cadencia de presentación");
    }
}
