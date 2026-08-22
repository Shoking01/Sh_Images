# Tasks: ffmpeg-real-decode

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 580-700 |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR1 open→ PR2 video→ PR3 audio/seek/ci |
| Delivery strategy | ask-on-risk |
| Chain strategy | stacked-to-main |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | PR | Focused test | Runtime | Rollback |
|------|------|----|--------------|---------|----------|
| 1 | open duration | PR1 | `cargo test --lib ffmpeg` | stub open | ffmpeg open |
| 2 | video sws | PR2 | `cargo test --features video` | rgba ignored | scaler |
| 3 | audio seek ci | PR3 | `cargo test --features video && cargo test` | lavfi 64x64 | Swr+seek+fixture |

## Phase 1: Foundation

- [x] 1.1 [CONFIG] `Cargo.toml` no `static` — est 0 — AC: no static — Dep: - — Test: `cargo check`
- [x] 1.2 [RED] `probe_dll`/`is_available` — est 10 — AC: stub toggles — Dep: 1.1 — Test: unit probe

## Phase 2: Core open (REQ-RD-001,006,009)

- [x] 2.1 [RED] `validate_file_header`+`check_codec` — est 10 — AC: empty/PNG/vp9→Media — Dep: 1.2 — Test: unit
- [x] 2.2 Owned `FfmpegDecoder` `Input/Context` — est 80 — AC: H264 fps>0 — Dep: 2.1 — Test: unit RED
- [x] 2.3 `refresh_duration`/`fps` — est 25 — AC: ±50ms — Dep: 2.2 — Test: unit

## Phase 3: Video (REQ-RD-002)

- [x] 3.1 [RED] RGBA `len==w*h*4` — est 10 — AC: RED — Dep: 2.2 — Test: integration
- [x] 3.2 `next_sample` av_read→SwsContext→Video — est 110 — AC: frames — Dep: 3.1 — Test: integration
- [x] 3.3 `Drop` scaler→ctx→ictx — est 10 — AC: 60f no leak — Dep: 3.2 — Test: ≤64MB

## Phase 4: Audio (REQ-RD-003)

- [x] 4.1 [RED] Audio f32 — est 10 — AC: RED — Dep: 3.2 — Test: integration
- [x] 4.2 `SwrContext` FLT — est 80 — AC: no Audio if video_only — Dep: 4.1 — Test: integration

## Phase 5: PTS/seek (REQ-RD-004,005,007)

- [x] 5.1 `pts_to_duration` — est 15 — AC: NOPTS 500ms saturate — Dep: 4.2 — Test: unit
- [x] 5.2 [RED] `seek` BACKWARD+flush — est 35 — AC: pts≥target — Dep: 5.1 — Test: integration
- [x] 5.3 `set_output_size` 25% — est 10 — AC: 1900 no 800 yes — Dep: 3.2 — Test: unit

## Phase 6: Fixture/CI (REQ-RD-008)

- [x] 6.1 Fixture <100KB gated — est 20 — AC: both pass — Dep: 4.2 — Test: cargo test
- [x] 6.2 CI `features video` LGPL — est 15 — AC: clippy green — Dep: 6.1 — Test: bash
- [x] 6.3 Docs MF→FFmpeg — est 5 — AC: no MF — Dep: 5.2 — Test: grep

## Phase 7: Cleanup

- [ ] 7.1 fmt FrameRing — est 5 — AC: clippy clean — Dep: 6.2 — Test: full
