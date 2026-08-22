//! Real FFmpeg decode tests (REQ-RD-002/003/005).
//!
//! Lives in its own integration-test TARGET on purpose: these tests require
//! the REAL decoder path (`SH_IMAGES_FFMPEG_REAL=1` + linked dev libs), while
//! lib unit tests flip the process-global TEST_FFMPEG_STUB atomic. Running
//! both in one process let stub tests force `allow_stub=true` and divert
//! `open()` away from the decoder mid-flight (observed as instant EOS on the
//! CI video lane). A separate target gives each suite its own process.
//!
//! Compiles empty without the `video` feature; every test early-returns when
//! the committed fixture is absent so machines without it stay green.

#![cfg(feature = "video")]

use std::time::Duration;

use sh_images::core::video::backend::{Decoder, Sample};
use sh_images::core::video::ffmpeg::FfmpegDecoder;

const FIXTURE_PATH: &str = "tests/fixtures/h264_64x64.mp4";
/// Drain bound: 2 s fixture = ~60 video frames + ~86 AAC chunks.
const FIXTURE_DRAIN_ROUNDS: usize = 4000;

fn real_fixture_available() -> bool {
    sh_images::core::video::ffmpeg::is_available() && std::path::Path::new(FIXTURE_PATH).exists()
}

#[test]
fn next_sample_real_decode_pixels_len_equals_wh4() {
    // REQ-RD-002 "Produces RGBA": decoded Sample::Video pixels length must
    // equal width*height*4 with pts >= ZERO.
    if !real_fixture_available() {
        return;
    }
    let mut dec = FfmpegDecoder::open(std::path::Path::new(FIXTURE_PATH), (64, 64), 48_000)
        .expect("fixture must open");
    match dec.next_sample().expect("decode must succeed") {
        Sample::Video(frame) => {
            assert_eq!(
                frame.pixels.len(),
                (frame.width * frame.height * 4) as usize
            );
            assert!(frame.pts >= Duration::ZERO, "pts must be monotonic");
        }
        other => panic!("expected Video sample, got {other:?}"),
    }
}

#[test]
fn next_sample_real_audio_is_f32_interleaved_at_device_rate() {
    // REQ-RD-003 S1: fixture AAC opened at device rate 48 kHz must yield
    // non-empty, pair-aligned, finite f32 with pts>=0.
    if !real_fixture_available() {
        return;
    }
    let mut dec = FfmpegDecoder::open(std::path::Path::new(FIXTURE_PATH), (64, 64), 48_000)
        .expect("fixture opens");
    let mut saw_video = false;
    let mut saw_audio = false;
    for _ in 0..FIXTURE_DRAIN_ROUNDS {
        match dec.next_sample().expect("decode must succeed") {
            Sample::Video(_) => saw_video = true,
            Sample::Audio { pts, samples } => {
                assert!(!samples.is_empty(), "audio chunks carry samples");
                assert_eq!(samples.len() % 2, 0, "stereo interleave keeps L/R pairs");
                assert!(
                    samples.iter().all(|&s| f32::is_finite(s)),
                    "f32 stays finite"
                );
                assert!(pts >= Duration::ZERO, "audio pts monotonic");
                saw_audio = true;
            }
            Sample::EndOfStream => break,
        }
    }
    assert!(saw_video && saw_audio, "fixture must yield both streams");
}

#[test]
fn open_video_only_real_never_emits_audio() {
    // REQ-RD-003 S2: no Audio variant may ever surface from open_video_only.
    if !real_fixture_available() {
        return;
    }
    let mut dec = FfmpegDecoder::open_video_only(std::path::Path::new(FIXTURE_PATH), (64, 64))
        .expect("fixture opens");
    assert!(!dec.info().has_audio);
    for _ in 0..FIXTURE_DRAIN_ROUNDS {
        match dec.next_sample().expect("decode must succeed") {
            Sample::Audio { .. } => panic!("video_only decoder emitted Audio"),
            Sample::Video(_) => {}
            Sample::EndOfStream => break,
        }
    }
}

#[test]
fn seek_real_decode_reaches_target_after_preroll_discard() {
    // REQ-RD-005 S1: BACKWARD lands on keyframe ≤ target (-g 30 ⇒ 1 s GOP);
    // decoding then reaches pts ≥ target without stale panics.
    if !real_fixture_available() {
        return;
    }
    let target = Duration::from_secs(1);
    let mut dec = FfmpegDecoder::open(std::path::Path::new(FIXTURE_PATH), (64, 64), 48_000)
        .expect("fixture opens");
    dec.seek(target).expect("seek succeeds");
    let mut reached = false;
    for _ in 0..FIXTURE_DRAIN_ROUNDS {
        match dec.next_sample().expect("decode must succeed") {
            Sample::Video(frame) if frame.pts >= target => {
                reached = true;
                break;
            }
            Sample::Video(_) => {}
            Sample::Audio { .. } => {}
            Sample::EndOfStream => break,
        }
    }
    assert!(reached, "decoding after seek must reach pts ≥ target");
}
