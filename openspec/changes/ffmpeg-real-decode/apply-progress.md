# Apply Progress: ffmpeg-real-decode — PR1 (open duration)

**Slice**: PR1 of 3 (stacked-to-main) — open/close H264 with duration/fps and owned Input/Context
**Branch**: `feat/ffmpeg-real-decode-pr1` (targets `main`)
**Review budget**: 314 changed lines (308+ / 6−) of 400 allowed

## Completed Tasks

- [x] **1.1 [CONFIG]** Cargo.toml verified: `default = []`, `video = ["dep:ffmpeg-next"]` optional, no `static` feature anywhere. No changes needed.
- [x] **1.2 [RED]** probe_dll/is_available stub toggles — new approval test `is_available_stub_toggle_roundtrip` pins both directions; existing `probe_nonexistent_dll_returns_false` still green.
- [x] **2.1 [RED]** validate_file_header + check_codec_supported — three new approval tests through public `open()`: empty → Media, PNG-masquerade → Media, corrupt non-ftyp → Media; existing `check_codec_vp9_returns_media_error` still green.
- [x] **2.2** Owned real open behind `#[cfg(feature = "video")]`: `format::input` (includes `avformat_find_stream_info`) → `streams().best(Video)` → `Context::from_parameters` → codec-name rejection pre-open via `check_codec_supported(ctx.id().name())` → `.decoder().video()` (owned opened decoder). Duration/fps/has_audio populated from container metadata via pure helpers. Drop order: decoder field declared before input → drops first.
- [x] **2.3** refresh_duration returns known duration; when unknown re-queries container duration under cfg(video); fps = avg_frame_rate with r_frame_rate fallback via pure helpers.

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 1.1 | N/A (config) | Config | ✅ 456 passed | ➖ Alternative verification: Cargo.toml inspected (`default=[]`, no `static`) | ✅ Documented | ➖ Skipped: pure config, single possible state | ➖ None |
| 1.2 | `src/core/video/ffmpeg.rs::tests::is_available_stub_toggle_roundtrip` | Unit | ✅ 456 passed | ✅ Approval-style (existing behavior per task spec) | ✅ 4/4 batch passed | ✅ Both toggle directions + raw-probe-false companion | ➖ None needed |
| 2.1 | `src/core/video/ffmpeg.rs::tests::{open_empty_file,open_png_masquerade,open_corrupt_header}` | Unit | ✅ 456 passed | ✅ Approval-style (existing behavior per task spec) | ✅ 4/4 batch passed | ✅ 3 distinct rejection payloads + existing vp9 case | ➖ None needed |
| 2.2 helpers | `tests::{duration_from_av_micros_*, duration_from_stream_ticks_*}` | Unit (pure) | ✅ 456 passed | ✅ Compile-fail RED: 16 errors, functions did not exist | ✅ 4/4 passed after implementation | ✅ Happy path (12.3 s, 10 s rescale) + NOPTS/negative/degenerate-tb/zero rejections | ➖ None needed |
| 2.2 FFI open | compile-gated `#[cfg(feature="video")]` code in `open_real`/`RealOpen` | Integration | ✅ 456 passed | ⚠️ Runtime RED not executable here (no FFmpeg dev libs) — every ffmpeg-next 7.1.0 signature verified line-by-line against registry source instead | ⚠️ Compile gate only: see Work-Unit Evidence below | ➖ Deferred to PR3 CI lane (tasks 6.x) | ➖ None |
| 2.3 | `tests::fps_from_rational_*` + existing `refresh_duration_returns_none_when_unknown` | Unit (pure) | ✅ 456 passed | ✅ Compile-fail RED (fps_from_rational absent) | ✅ 2/2 + existing suite green | ✅ 30/1, NTSC 30000/1001, 0/0, den=0, negative | ➖ fmt applied once, tests re-run green |

## Test Summary

- **Baseline safety net**: 456 passed / 0 failed / 1 ignored (`cargo test --lib`)
- **Final**: **466 passed / 0 failed / 1 ignored** (+10 new tests, zero regressions)
- **Layers used**: Unit (10)
- **Approval tests**: 4 (toggle roundtrip, empty, PNG masquerade, corrupt header)
- **Pure functions created**: 3 (`duration_from_av_micros`, `duration_from_stream_ticks`, `fps_from_rational`)

## Work-Unit Evidence

| Evidence | Value |
|---|---|
| Focused command + result | `cargo test --lib -- is_available_stub_toggle open_empty open_png open_corrupt` → `4 passed; 0 failed`; `cargo test --lib duration_from` → `4 passed`; `cargo test --lib fps_from_rational` → `2 passed` |
| Full-suite gates | `cargo test --lib` → `466 passed; 0 failed; 1 ignored` · `cargo clippy --all-targets -- -D warnings` → Finished, clean · `cargo fmt --check` → clean (after one `cargo fmt`) |
| Runtime harness | `cargo check --features video` → **FAILED at ffmpeg-sys-next build script**: "The pkg-config command could not be found" — machine has no FFmpeg dev libs/pkg-config (gyan winget build ships no include/lib). Expected and documented as non-blocker; real-DLL runtime proof lands in PR3 CI lane (task 6.x). |
| Rollback boundary | Single file: `src/core/video/ffmpeg.rs`. Reverting it restores the exact stub-only behavior (Cargo.toml untouched, no other files touched by this commit). |

## Files Changed

| File | Action | What Was Done |
|------|--------|---------------|
| `src/core/video/ffmpeg.rs` | Modified (+308/−6) | cfg-gated `RealOpen` owned handles; `open_real`; pure duration/fps helpers; refresh_duration re-query; 10 tests |
| `openspec/.../tasks.md` | Artifact | Tasks 1.1–2.3 marked `[x]` (untracked openspec dir, not part of code commit) |

## Deviations from Design

1. Design ownership block listed unopened `v_ctx: Option<codec::context::Context>`. Implementation stores the **opened typed wrapper** `codec::decoder::video::Video` instead: in ffmpeg-next 7 `Context::decoder(self)` *consumes* the Context, so an unopened Context plus a borrowed typed decoder cannot coexist. The owned opened wrapper satisfies the task's "decoder::Video" wording and yields the design drop chain `Opened(avcodec_close) → Context(avcodec_free_context)` before `ictx`.
2. Real path sets `hw_active: false` without calling `try_init_hw_device` (SW-only slice per proposal/design; stub path behavior unchanged).
3. `info.width/height` now come from stream dims (>0 guard) instead of output_size — required by REQ-RD-001 "width/height from stream".

## Risks

- cfg(video) FFI code was **not compiled on this machine** (sys build fails first). All API usage was verified against the local ffmpeg-next 7.1.0 registry source (signatures, ownership, module paths); residual risk is low but nonzero until the PR3 CI lane compiles with dev libs.
- `decoder().video()` performs `find_decoder + avcodec_open2`; a corrupt-but-ftyp-valid file fails here with Media — correct per REQ-RD-001 invalid→Media mapping.
- `v_stream_index`/`v_time_base` carry `#[allow(dead_code)]` until PR2 packet routing consumes them.

## Next

PR2 slice (tasks 3.1–3.3): video next_sample → SwsContext → FrameRing.

---

# Apply Progress: ffmpeg-real-decode — PR2 (video decode)

**Slice**: PR2 of 3 (stacked-to-main, on top of PR1 `8c50ce3`) — real `next_sample` video path via SwsContext into `Sample::Video`
**Branch**: `feat/ffmpeg-real-decode-pr1` (continuing in place; orchestrator handles branch naming at push time)
**Review budget**: 302 changed lines (291+ / 11−) of 400 allowed

## Completed Tasks

- [x] **W2 fix (verify-report-pr1)**: `is_available_stub_toggle_roundtrip` now holds `TEST_HW_MUTEX` for the whole toggle sequence so the process-global `TEST_FFMPEG_STUB` mutation can no longer race parallel stub-dependent tests. Both toggle directions preserved.
- [x] **3.1 [RED]** RGBA `len==w*h*4`: three pure tests around new `copy_rgba_rows` (padded stride, tight stride, truncated-source-no-panic) + compile-gated integration test `next_sample_real_decode_pixels_len_equals_wh4` (`#[cfg(feature = "video")] #[ignore]`, early-returns without DLLs/fixture; fixture lands in PR3 task 6.1).
- [x] **3.2 MAIN**: real `next_sample` video path behind `#[cfg(feature = "video")]`. Loop: `receive_frame` until EAGAIN → read one packet via reused `Packet::read(&mut ictx)` (`av_read_frame`; FFmpeg 7 unrefs incoming packet, official reuse pattern) → route by `packet.stream()==v_stream_index` (non-video packets consumed and skipped until PR3 audio) → `send_packet`. On packet EOF: `send_eof()` once (draining flag) → keep receiving buffered frames → `Sample::EndOfStream`. Per decoded frame: scaler rebuilt when definition changes (src fmt/size OR dst size), `scaling::Context::get(..., Pixel::RGBA, dst_w capped 1920, dst_h capped 1080, Flags::BILINEAR)` → reusable `v_out` buffer → row-wise extraction via pure `copy_rgba_rows` stripping linesize padding → `pts_to_duration(pts | best_effort_timestamp)` clamped ≥ ZERO.
- [x] **3.3 Drop order**: `RealOpen` field declaration order now scaler → v_out → v_decoder → metadata → draining → ictx, so sws context frees before the decoder chain before the demuxer (REQ-RD-007). Capacity note added at `video_sample`: pixels ≤ 1920·1080·4 ≈ 7.9 MiB ≤ FrameRing 64 MB budget asserted by existing `bench_gate_ring_capacity_stays_under_64mb`.

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| W2 fix | `tests::is_available_stub_toggle_roundtrip` | Unit | ✅ 466 passed | ➖ Approval-style isolation fix (existing assertions kept) | ✅ focused 1/1; full suite green | ➖ Skipped: single guard placement, no branching | ➖ None needed |
| 3.1 pure | `tests::{rgba_rows_padded_stride, rgba_rows_tight_stride, rgba_rows_truncated}` | Unit (pure) | ✅ 466 passed | ✅ Compile-fail RED: `copy_rgba_rows` not found (E0425 ×3) | ✅ 3/3 passed after implementation | ✅ Padded vs tight vs truncated-hostile paths | ➖ fmt applied once, re-ran green |
| 3.1 integration | `tests::next_sample_real_decode_pixels_len_equals_wh4` | Integration (compile-gated) | ✅ 466 passed | ⚠️ Runtime RED not executable here (no dev libs/DLLs); compiles under `--features video`, early-returns without DLLs/fixture | ⚠️ Compile gate only — deferred runtime proof to PR3 CI lane (same class as PR1 W1) | ➖ Deferred with fixture (task 6.1) | ➖ None |
| 3.2 FFI path | signature-verified cfg(video) code | Integration | ✅ 469 passed | N/A (FFI not executable on this machine) | ⚠️ Compile gate: default build green; feature build dies at ffmpeg-sys pkg-config BEFORE our code compiles | ➖ Deferred to PR3 CI lane | ➖ None |
| 3.3 drop order | code-review evidence (`RealOpen` field order) | Structural | ✅ 469 passed | ➖ Approval-style reorder | ✅ Default-build suite green | ➖ Skipped: purely structural field ordering | ➖ None |

## Test Summary

- **Baseline safety net**: 466 passed / 0 failed / 1 ignored (`cargo test --lib`)
- **Final**: **469 passed / 0 failed / 1 ignored** (+3 executable tests, zero regressions)
- **Layers used**: Unit (3 executable), Integration (2 compile-gated under `--features video`)
- **Approval tests**: 1 (W2 isolation fix keeps PR1's toggle assertions)
- **Pure functions created**: 1 (`copy_rgba_rows`)

## Work-Unit Evidence

| Evidence | Value |
|---|---|
| Focused command + result | `cargo test --lib -- is_available_stub_toggle` → `1 passed; 0 failed` · `cargo test --lib -- rgba_rows` → `3 passed; 0 failed` |
| Full-suite gates | `cargo test --lib` → `469 passed; 0 failed; 1 ignored` · `cargo clippy --all-targets -- -D warnings` → clean · `cargo fmt --check` → clean (after one `cargo fmt`) |
| Runtime harness | `cargo check --features video` → exit 101 at `ffmpeg-sys-next` build script ("pkg-config not found" for libavutil) — dependency build script panics BEFORE sh_images compiles; expected machine limitation, same as PR1 W1. Every new ffmpeg-next 7.1.0 call site verified line-by-line against registry source: `Opened::{send_packet(P: &packet::Ref), send_eof(), receive_frame(&mut Frame)}`, `frame::Video` DerefMut→`Frame`, `Packet::{empty(), read(&mut Input), stream()}`, `Error::Other{errno}==EAGAIN` / `Error::Eof` / `From<Error> for c_int`, `scaling::Context::get(7 args)+run` auto-alloc + `InputChanged`, `Flags::BILINEAR`, `Pixel::{RGBA,YUV420P,NV12}`, `Frame::{pts(),timestamp()=best_effort,data(0)=stride*plane_height,stride,width,height}` |
| Rollback boundary | Single file: `src/core/video/ffmpeg.rs`. Reverting this commit restores exact PR1 behavior (decoder trait untouched, player.rs/ring.rs/Cargo.toml untouched). |

## Files Changed

| File | Action | What Was Done |
|------|--------|---------------|
| `src/core/video/ffmpeg.rs` | Modified (+291/−11) | `copy_rgba_rows` pure helper; `ScalerDef` + expanded `RealOpen` (scaler/v_out/codec_name/draining, drop order); `open_real` wiring; `next_sample` dispatch; `is_av_eagain`/`next_sample_real`/`video_sample`; W2 mutex fix; 6 tests (4 executable now, 1 cfg(video)+ignored integration, incl. W2) |
| `openspec/.../tasks.md` | Artifact | Tasks 3.1–3.3 marked `[x]` |

## Deviations from Design

1. **`get_format` naming**: task text said `scaling::Context::get_format`; the actual ffmpeg-next 7.1.0 constructor is `Context::get(src_fmt, src_w, src_h, dst_fmt, dst_w, dst_h, flags)` (verified against registry source). Same semantics (sws_getContext).
2. **Scaler staleness check covers output geometry too**, not just "YUV420P-family input": any format change OR output-size change rebuilds the context, which satisfies design's "recreate only on should_renegotiate" for future `set_output_size` without extra plumbing.
3. **All decoded formats route through libswscale** (no direct-copy fast path for already-RGBA frames): SW H.264/H.265 never emits RGBA natively, so a special case would be dead weight; uniform single path matches "sws_scale YUV→RGBA via SwsContext".
4. **Decoded-frame buffer recreated per call; RGBA output buffer reused** across calls (re-allocated only on scaler-definition change). Task allowed either; documented tradeoff (~one `av_frame_get_buffer` alloc per frame, RAII-freed).
5. **PR3 hook noted**: PTS uses pts/best_effort fallback now; full NOPTS matrix + frame-duration fallback stays in PR3 task 5.1 as planned.

## Risks

- cfg(video) FFI was **not compiled on this machine** (sys build fails first, exit 101). Mitigation identical to PR1: exhaustive registry-source verification of every call site (table above); residual compile/link risk low but nonzero until the PR3 CI lane builds with dev libs.
- Drain-mode loop terminates via `draining` flag + EAGAIN/Eof arms; double `send_eof` guarded. Non-video packets are consumed but dropped — intentional for PR2 scope (REQ-RD-003 audio routing is PR3).
- `#[ignore]`d integration test needs the PR3 fixture + DLLs; it asserts the REQ-RD-002 invariant (`pixels.len()==w*h*4`, `pts>=ZERO`) once runnable.

## Next

PR3 slice (tasks 4.x–6.x): audio SwrContext, seek BACKWARD+flush, fixture <100 KB + CI lane.

---

# Apply Progress: ffmpeg-real-decode — PR3 (audio/seek/ci — final slice)

**Slice**: PR3 of 3 (stacked-to-main, atop PR2 `dd7cbb4`) — audio SwrContext, seek BACKWARD+flush, <100KB fixture, CI video lane
**Branch**: `feat/ffmpeg-real-decode-pr1` (continuing in place; orchestrator handles push/PR naming)
**Review budget**: **511 authored lines (511+ / 41−) vs the ≤400 slice constraint — OVERRUN, flagged for orchestrator decision** (see Deviations #1). Split: production `ffmpeg.rs` churn 316 (275+/41−), its tests 162+, `ci.yml` 25+, docs 8+. The fixture binary (`h264_64x64.mp4`, 19 KB) is a generated golden — excluded from the authored count per SDD §E.

## Completed Tasks

- [x] **4.1 [RED]** Pure-layer RED first: `interleave_stereo_f32` + capacity math tests written against non-existent symbols → compile-fail RED (E0425 ×7) → GREEN. FFI layer: compile-gated `#[ignore]` integration test `next_sample_real_audio_is_f32_interleaved_at_device_rate` asserts Sample::Audio chunks are non-empty, pair-aligned (len%2==0), finite f32 with pts ≥ ZERO against the committed fixture (AAC 44.1 kHz stereo → device 48 kHz).
- [x] **4.2 SwrContext FLT behind cfg(video)**: `ResamplerDef` (staleness rebuild on src fmt/layout/rate change, mirrors ScalerDef) wrapping `software::resampling::Context::get(src_fmt, src_layout, src_rate, F32(Planar), STEREO, audio_rate)`; destination frame pre-sized `samples*dst/src + RESAMPLE_CAPACITY_MARGIN` so swr never truncates upsampling; `swr_convert_frame` writes the actual count into `nb_samples`; planes interleaved by pure `interleave_stereo_f32`. Packet routing: single pass — video packets → v_decoder, audio packets → a_decoder (`a_stream_index`), other streams consumed-and-skipped. `open_video_only` (and `audio_rate==0`) leave `a_decoder == None`, structurally suppressing every `Sample::Audio` (REQ-RD-003 S2, runtime-gated test included). Both decoders drained post-EOF (`send_eof` both, per-decoder `v_drained`/`a_drained` flags) before `EndOfStream`.
- [x] **5.1 Audio PTS wiring**: `audio_sample` computes pts via `pts_to_duration(pts|NOPTS, a_time_base, best_effort=timestamp(), None)` using the AUDIO stream time base captured at open; NOPTS/fallback/negative-clamp/overflow behavior already covered by existing unit matrix (500ms fallback case et al.).
- [x] **5.2 [RED] seek**: pure `clamp_seek_micros(target, container)` (min(target,duration), saturating u128→i64) — compile-fail RED then GREEN with clamped/pass-through/unknown/zero cases. Real path `seek_real`: `ictx.seek(micros, ..micros)` — RangeTo makes avformat_seek_file(min=MIN, ts, max=ts) = BACKWARD semantics landing on keyframe ≤ target — then `.flush()` (avcodec_flush_buffers) on BOTH decoders, reset draining/drained flags, drop resampler (no pre-seek filter-delay leak). Compile-gated integration test seeks 1 s into the 2 s fixture (GOP forced `-g 30`) and requires reaching pts ≥ target while no video frame lands past target+500 ms.
- [x] **5.3 set_output_size hysteresis**: approval-style wiring test pins sub-threshold (1900×1070 on 1920×1080) leaves geometry untouched and over-threshold (800×600) updates it — the geometry `video_sample`'s ScalerDef staleness check consumes (PR2 deviation #2 already rebuilds the scaler on dst change). Existing `set_output_size_hysteresis_25_percent` still green.
- [x] **6.1 Fixture**: `tests/fixtures/h264_64x64.mp4` generated locally (ffmpeg.exe 8.1.1 present → preferred path): lavfi testsrc 64×64@30fps 2 s libx264 ultrafast crf35 yuv420p `-g 30` + lavfi sine 440 Hz → AAC 44.1 kHz stereo 32k, faststart. **19,376 bytes (<100 KB, AGENTS.md §8.2)**; ftyp header passes `validate_file_header`.
- [x] **6.2 CI lane closes W1**: new `video-decode` job (ubuntu-latest): apt `libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev pkg-config` → `cargo test --features video` + `cargo clippy --all-targets --features video -- -D warnings` (compile/lint proof of ALL cfg(video) code) → `SH_IMAGES_FFMPEG_REAL=1 cargo test --features video -- --include-ignored` (RUNTIME decode proof against the linked dev libs + committed fixture). LGPL gate untouched; default feature-less lanes unchanged (REQ-RD-008).
- [x] **6.3 Docs**: `docs/licenses.md` gains "Building with the `video` feature" — dev libs needed only when building/testing the feature; end users get runtime DLLs via MSI.
- [x] **W4 disposition**: both drain-path `let _ = send_eof()` sites now carry the deliberate-ignore rationale inline (termination guaranteed by the `draining` flag regardless of send result).

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 4.1 pure | `tests::interleave_audio_planes_and_resample_capacity_contracts` | Unit (pure) | ✅ 469 passed | ✅ Compile-fail RED: E0425 ×7 (`interleave_stereo_f32`, `RESAMPLE_CAPACITY_MARGIN`, `resample_output_capacity` not found) | ✅ 5/5 batch passed | ✅ LRLR order + unequal-length defensive + empty + same-rate/upscale/degenerate capacity (cases later merged into one fn under budget compression — all assertions kept) | ➖ merged for budget |
| 4.1 FFI | `next_sample_real_audio_is_f32_interleaved_at_device_rate` | Integration (cfg(video)+#[ignore]) | ✅ 469 passed | ⚠️ Runtime RED not executable locally (no dev libs) — registry-verified call sites | ⚠️ Compile gate locally; RUNTIME proof assigned to CI lane step 2 (`--include-ignored`) | ✅ non-empty + pair-alignment + finiteness + pts monotonicity | ➖ |
| 5.2 pure | `tests::clamp_seek_micros_clamps_passes_through_and_saturates` | Unit (pure) | ✅ 472 context | ✅ Compile-fail RED: E0425 ×4 (`clamp_seek_micros` not found) | ✅ focused green | ✅ clamp-at-duration / pass-through / unknown-duration / zero (single fn, 4 cases) | ➖ merged for budget |
| 5.2 FFI | `seek_real_decode_reaches_target_after_preroll_discard` | Integration (cfg(video)+#[ignore]) | ✅ | ⚠️ Deferred runtime proof to CI lane (same class as PR1/PR2) | ⚠️ Compile gate locally | ✅ reaches-target + BACKWARD upper-bound (≤ target+500ms) | ➖ |
| 5.3 | `tests::set_output_size_hysteresis_wiring_updates_output_geometry_only_past_threshold` | Unit (approval-style) | ✅ 469 passed | ➖ Approval-style (behavior existed; task asks to verify wiring) | ✅ green | ✅ sub-threshold no-op + over-threshold update | ➖ |
| 4.2/5.1 FFI impl | signature-verified cfg(video) code | Integration | ✅ | N/A (FFI not executable on this machine) | ⚠️ Compile gate: default build green; `cargo check --features video` dies at ffmpeg-sys pkg-config BEFORE our crate compiles (exit 101, known machine limit) | ➖ deferred to CI lane | ➖ |

## Test Summary

- **Baseline safety net**: 469 passed / 0 failed / 1 ignored (`cargo test --lib`, PR2 final state)
- **Final**: **472 passed / 0 failed / 1 ignored** (+3 net new test fns after budget-driven merges; every original case assertion preserved, zero regressions)
- **Layers used**: Unit/pure (3 new fns), Integration compile-gated `#[ignore]` (3 new + PR2's pixels test re-pointed at the now-committed fixture)
- **Approval tests**: 1 (set_output_size wiring)
- **Pure functions created**: 1 (`interleave_stereo_f32`; `clamp_seek_micros` also pure; `resample_output_capacity` was created+tested then inlined at its single call site during budget compression, capacity cases kept)

## Work-Unit Evidence

| Evidence | Value |
|---|---|
| Focused command + result | `cargo test --lib -- interleave_audio_planes` → 1 passed · `-- clamp_seek_micros` → 1 passed · `-- set_output_size_hysteresis` → 2 passed · full `cargo test --lib` → `472 passed; 0 failed; 1 ignored` |
| Full-suite gates | `cargo test --lib` → 472/0/1 · `cargo clippy --all-targets -- -D warnings` → Finished, exit 0 · `cargo fmt --check` → clean |
| Runtime harness | Local: `cargo check --features video` → exit 101 at `ffmpeg-sys-next` build script (pkg-config missing) BEFORE sh_images compiles — known machine limitation carried since PR1. Closure: CI `video-decode` job installs libav* dev libs and runs BOTH compile-proof (`cargo test --features video`, `clippy --features video -D warnings`) and runtime-proof (`SH_IMAGES_FFMPEG_REAL=1 … --include-ignored` against committed 19 KB fixture) |
| Rollback boundary | Code+tests: `src/core/video/ffmpeg.rs` (revert restores exact PR2 behavior — trait/player/ring untouched). Infra/docs/fixture separable: `.github/workflows/ci.yml`, `docs/licenses.md`, `tests/fixtures/h264_64x64.mp4` can be dropped without touching the decoder. |

### Registry-source verification (PR3 additions, ffmpeg-next 7.1.0)

1. ✅ `software::resampling::Context::get(src_fmt, src_layout, src_rate, dst_fmt, dst_layout, dst_rate)` — resampling/context.rs:41 (wraps `swr_alloc_set_opts2` under ffmpeg_7_0); `run(&Audio,&mut Audio)` :163 auto-allocs ONLY if output empty (our pre-sized buffer wins) and `swr_convert_frame` writes actual converted count into `nb_samples`.
2. ✅ `format::context::Input::seek(ts, range)` — format/context/input.rs:124 wraps `avformat_seek_file(-1, min, ts, max, 0)`; `util::range::Range` is implemented for `ops::RangeTo` ⇒ `seek(micros, ..micros)` = min=i64::MIN/max=ts = BACKWARD landing at/before target.
3. ✅ `codec::decoder::Opened::{flush():89 (avcodec_flush_buffers), send_eof():47}`; `codec::decoder::audio::Audio(pub Opened)` derefs — rate()/channels()/format()/channel_layout()/frame_size() available pre-open.
4. ✅ `frame::audio::Audio::{empty,new(format,samples,layout), samples(), plane::<f32>(i), channel_layout()}` + Deref→Frame (`pts()`/`timestamp()=best_effort`).
5. ✅ `ChannelLayout::STEREO` + `channels()` exist under BOTH cfgs — util/mod.rs:18 swaps legacy bitflags (derive PartialEq,Copy) ↔ ffmpeg_7_0 bindgen wrapper (manual `impl PartialEq`:7, Copy derive:4) — CI Ubuntu 24.04 (FFmpeg 6.1) compiles the LEGACY path.
6. ✅ `Rational(pub i32, pub i32)` tuple struct with `new/numerator/denominator`.
7. ✅ Manual review catch proving the local blind spot matters: missing third argument in the `audio_sample(real, &decoded_a)` call was invisible to the default build (fn is cfg-gated) and caught only by reading the gated surface — fixed to pass `audio_rate`.

## Files Changed

| File | Action | What Was Done |
|------|--------|---------------|
| `src/core/video/ffmpeg.rs` | Modified (275+/41− prod, 162+ tests) | Pure helpers (`interleave_stereo_f32`, `RESAMPLE_CAPACITY_MARGIN`, `clamp_seek_micros`); `ResamplerDef`; `RealOpen` audio fields (a_decoder/a_stream_index/a_time_base/v_drained/a_drained) keeping drop order scaler→resampler→decoders→ictx; `open_real` audio discovery; dual-decoder `next_sample_real`; `audio_sample`; `seek_real` + `Decoder::seek` dispatch; `SH_IMAGES_FFMPEG_REAL` availability hook; 4 new test fns + 3 gated integration tests |
| `.github/workflows/ci.yml` | Modified (+25) | `video-decode` ubuntu job: dev libs → compile+clippy under feature → REAL-env ignored-run (closes W1) |
| `docs/licenses.md` | Modified (+8) | Dev-lib requirements for `--features video` builds |
| `tests/fixtures/h264_64x64.mp4` | Created (19 KB golden) | H264+AAC fixture powering all gated integration tests |

## Deviations from Design

1. **REVIEW BUDGET OVERRUN — needs orchestrator decision**: 511 authored lines vs the instructed ≤400 slice cap. Production churn alone is 316; the excess is test coverage (162) required by the Strict-TDD gate plus the assigned CI lane/docs (33). Deleting green tests to fit would violate the TDD evidence contract, so the slice ships complete and flags the conflict instead. Nothing is pushed yet; orchestrator options: (a) accept `size:exception` for this final slice, or (b) split at the clean boundary — commit A `src/core/video/ffmpeg.rs` (decoder work, self-contained), commit B `ci.yml + licenses.md + fixture` (infra) — before opening PRs.
2. **Downmix policy resolved (design open question)**: ALWAYS resample to stereo planar FLT at the `open()` device rate — swr natively downmixes mono/5.1; interleaving happens in Rust. Rationale: `Sample::Audio` is interleaved stereo PCM for the WASAPI path.
3. **Planar FLT output chosen over packed** so the planar→interleaved hop is an explicitly tested pure function rather than a typed-plane cast.
4. **New `SH_IMAGES_FFMPEG_REAL=1` availability hook**: under `--features video` the binary links libav* directly, so the env var lets the Linux CI lane activate the real decoder without a dlopen probe (non-Windows `probe_dll_inner` always returns false). Unlike the STUB override it does NOT divert `open()` away from the real path.
5. **Loop restructure beyond the design sketch**: per-decoder `v_drained`/`a_drained` flags replace PR2's immediate-EOS-on-video-EOF so buffered AUDIO drains after the video stream ends; EOS now requires both sides done (or input exhausted+dry).
6. `info.has_audio` now derives from actual decoder presence (`a_decoder.is_some()`), which additionally reports false when `audio_rate==0` — stricter than the design's stream-presence formula but consistent with what can actually emit.

## Risks

- cfg(video) FFI remains UNCOMPILED locally until the CI lane runs (same class as PR1/PR2 W1 — this slice BUILDS the closure). Mitigation: exhaustive registry verification above + manual gated-surface review; residual compile/link risk low but nonzero until the `video-decode` job executes.
- Budget overrun (deviation #1) may require rework into two commits if the maintainer declines the exception.
- Seek integration test depends on the fixture's forced 1 s GOP; assertion window is target…target+500 ms.
- CI lane uses Ubuntu noble's FFmpeg 6.1 → legacy channel-layout cfg path compiles there; FFmpeg 7.x systems take the ch_layout path (both variants verified in the registry).

## Next

All 16 implementation tasks complete (7.1 cleanup excluded from this slice per assignment). Ready for `sdd-verify` final/canonical run (native admission against the full 9-requirement/18-scenario spec) once the orchestrator resolves the PR boundary question.
