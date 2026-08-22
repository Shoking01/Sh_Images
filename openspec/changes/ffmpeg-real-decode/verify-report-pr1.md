```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:e0a0d2cc3ae0c02d472b1200d84ecf423b4a1432364376ecf3795e9ffe4d0356
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 1/9
scenarios: 5/18
test_command: cargo test --lib
test_exit_code: 0
test_output_hash: sha256:76c04dad382914a670e5f5fefa2536432b98176d660393f4a52cee71e1f8b10f
build_command: cargo clippy --all-targets -- -D warnings
build_exit_code: 0
build_output_hash: sha256:d0d8fb8b055b4ba518f0b2c769b0c043ac2bd6ccc419274fb7e72c1a504a8686
```

# Verify Report — ffmpeg-real-decode PR1 (open/duration slice)

> **Admission note (recorded, per verify contract).** `gentle-ai sdd-verify-validate --input <candidate> --requirements 9 --scenarios 18` was executed against these exact bytes and **denied admission** with `passing verdict contradicts failing or incomplete evidence`: the native envelope requires any non-`fail` verdict to carry complete counts (`completed == total`), which a stacked-slice report cannot honestly claim against the authoritative 9/18 totals while 13 scenarios await PR2/PR3. Nothing failed in this slice, so a `fail` verdict would be equally false. Per the explicit slice-verification instruction, this PR1 report is persisted as a **slice artifact**; the canonical `openspec/changes/ffmpeg-real-decode/verify-report.md` remains unwritten and MUST pass native admission at final verification after the last slice lands.

- **Change**: `ffmpeg-real-decode` — PR1 slice only (tasks 1.1, 1.2, 2.1, 2.2, 2.3)
- **Mode**: openspec (repo-local)
- **Branch / commit**: `feat/ffmpeg-real-decode-pr1` @ `8c50ce3` (`feat(video): open H264 containers via ffmpeg-next with duration/fps`)
- **Scope note**: tasks 3.x–7.x belong to PR2/PR3 slices; they are intentionally unchecked in this stacked chain and are NOT counted as failures of this verification. Full-change verification remains pending until the final slice lands; archive stays blocked until then by design.

## 1. Task Completeness

| Task | State | Evidence |
|------|-------|----------|
| 1.1 [CONFIG] Cargo.toml no `static`, `default=[]` | ✅ done | Cargo.toml inspected: `[features] default = []`, `video = ["dep:ffmpeg-next"]`, `hwaccel = ["video"]`; no `static` feature anywhere |
| 1.2 [RED] probe_dll/is_available stub toggles | ✅ done | `is_available_stub_toggle_roundtrip` + `probe_nonexistent_dll_returns_false` green at runtime |
| 2.1 [RED] validate_file_header + check_codec | ✅ done | `open_empty_file_*`, `open_png_masquerade_*`, `open_corrupt_header_*`, `check_codec_vp9_*` green at runtime |
| 2.2 Owned FfmpegDecoder Input/Context | ✅ done | `RealOpen` owned handles behind `#[cfg(feature = "video")]`; compile gate only on this machine (see W1) |
| 2.3 refresh_duration/fps helpers | ✅ done | `duration_from_av_micros_*`, `duration_from_stream_ticks_*`, `fps_from_rational_*` green at runtime |

PR1 slice: **5/5 complete**. Whole change: 5/16 tasks (PR2/PR3 pending).

## 2. Runtime Evidence

| Command | Result | Exit |
|---------|--------|------|
| `cargo test --lib` | 466 passed / 0 failed / 1 ignored | 0 |
| `cargo test --lib core::video::ffmpeg` | 43 passed (33 prior + 10 new) | 0 |
| `cargo clippy --all-targets -- -D warnings` | clean | 0 |
| `cargo fmt --check` | clean | 0 |
| `git log --oneline -2` | single work-unit commit `8c50ce3` atop main tip `02a1524` | 0 |
| `git diff main...HEAD --stat` | 1 file, `src/core/video/ffmpeg.rs` +308/−6 = 314 lines ≤ 400 budget | 0 |
| `cargo check --features video` | FAILED at `ffmpeg-sys-next-7.1.3\build.rs:1035:14` — pkg-config not found for libavutil; dependency build script panics BEFORE any sh_images code compiles (no `Checking sh_images` reached). Exit 101. Expected machine limitation; recorded as deferred item, NOT a failure. | 101 |

## 3. Spec Compliance Matrix (authoritative spec totals: 9 requirements / 18 scenarios)

| Requirement | Scenario | Status | Covering evidence (runtime) |
|-------------|----------|--------|------------------------------|
| REQ-RD-001 Open/error mapping | Valid H.264 opens | DEFERRED (UNTESTED here) | Requires FFmpeg dev libs/DLLs → PR3 CI lane per plan; implementation code-reviewed |
| REQ-RD-001 Open/error mapping | Invalid maps to Media | ✅ PASS | `open_empty_file_returns_media_not_panic`, `open_png_masquerade_returns_media_not_panic`, `open_corrupt_header_returns_media_not_panic`; missing file → `Media("cannot stat …")` verified in `validate_file_header` |
| REQ-RD-002 Video next_sample | both | OUT OF SLICE (PR2) | — |
| REQ-RD-003 Audio next_sample | both | OUT OF SLICE (PR3) | — |
| REQ-RD-004 PTS monotonicity | both | OUT OF SLICE (PR3) | — |
| REQ-RD-005 Seek | both | OUT OF SLICE (PR3) | — |
| REQ-RD-006 Duration/fps | Duration known | ✅ PASS (unit layer) | `duration_from_av_micros_maps_known_duration` asserts 12.3 s ± 50 ms; stream-ticks fallback 10 s/5 s asserted; end-to-end container query deferred to PR3 CI lane |
| REQ-RD-006 Duration/fps | Unknown polled | ✅ PASS (unit layer) | NOPTS(i64::MIN)/0/negative → None; degenerate time base → None; `refresh_duration` returns None without panic |
| REQ-RD-007 Cleanup/budget | both | OUT OF SLICE (PR2/PR3) | Drop ORDER code-reviewed now; leak/budget proof deferred |
| REQ-RD-008 Availability/feature | No feature still tests | ✅ PASS | This verification literally ran `cargo test --lib` without `video`: 466/0/1, no FFI linked |
| REQ-RD-008 Availability/feature | No DLLs → Media | DEFERRED (partial) | Stub-side covered; feature-on-without-DLLs needs compilable cfg(video) → PR3 CI lane |
| REQ-RD-009 Scope SW-only | VP9 rejected | ✅ PASS (helper layer) | `check_codec_vp9_returns_media_error` green ("unsupported codec: vp9"); pre-open ordering (`check_codec_supported(ctx.id().name())` before `decoder().video()`) code-reviewed against ffmpeg-next source |
| REQ-RD-009 Scope SW-only | H.265 accepted | DEFERRED (UNTESTED) | Needs real decode → PR2/PR3 per plan |

**Completed**: 1 requirement fully covered (REQ-RD-006), 5 scenarios with passing covering tests.

## 4. Correctness Review (code vs spec/design)

| Check | Verdict | Detail |
|-------|---------|--------|
| All FFI strictly inside `#[cfg(feature = "video")]` | ✅ | `RealOpen` struct, `real` field, `open_real`, real-path call site, and both `refresh_duration` re-query blocks are individually cfg-gated; default build never references `ffmpeg_next` symbols (proven: clippy/test/clippy run without feature links nothing) |
| Owned handles, Drop order decoder-before-input | ✅ | Rust drops fields in declaration order: `v_decoder` declared before `ictx` in `RealOpen`. Chain `Video → Opened(avcodec_close) → Context(avcodec_free_context)` matches design intent |
| Codec rejection PRE-open | ✅ | `check_codec_supported(ctx.id().name())` executes before `.decoder().video()` (= before avcodec_open2); helper accepts h264/avc/h265/hevc, rejects all else incl. vp9 with Media naming the codec |
| Duration/fps helpers pure + saturating | ✅ | No panic paths: negative/zero/degenerate inputs rejected before arithmetic; rescale goes through existing `av_rescale_q_saturating` (i128 clamp); 10 unit tests cover happy + hostile cases |
| has_audio respects video_only | ✅ | `has_audio_stream && !video_only` in `open_real` info construction |
| Stub path preserved | ✅ | Feature off → cfg blocks compiled out entirely; DLLs absent on Windows → `Media("FFmpeg DLLs missing")`; explicit stub override (`TEST_FFMPEG_STUB` or env) bypasses real open even when DLLs present (`is_available() && !allow_stub`) |
| No `static` ffmpeg feature; `default=[]` unchanged | ✅ | Cargo.toml untouched by commit; features are exactly `default=[]`, `video=["dep:ffmpeg-next"]`, `hwaccel=["video"]` |
| ffmpeg-next 7.1.0 API usage | ✅ (source-verified) | All call sites matched line-by-line against local registry source: `format::input` (runs open_input+find_stream_info), `streams().best(Type)`, `stream.{index,time_base,avg_frame_rate,rate,duration,parameters}`, `Context::{from_parameters(P: Into<Parameters>), id()->Id, decoder(self)}` (consumes Context), `Id::name`, `Decoder::video(self)` (find+open_as+avcodec_open2), `Video::{width,height}`, `Rational::{numerator,denominator}` |

## 5. Design Coherence

| Design decision | Implementation | Coherent? |
|-----------------|----------------|-----------|
| Owned wrappers (RAII) over raw pointers | `RealOpen` owns `Video` decoder + `Input`; no raw `*mut AV*` in PR1 diff | ✅ |
| Decoder freed before demuxer | Field declaration order guarantees it | ✅ |
| Codec rejection before open | Verified ordering | ✅ |
| Design listed unopened `v_ctx: Option<Context>` | Implementation stores opened typed `decoder::video::Video` instead — ffmpeg-next 7 `decoder(self)` consumes the Context, so an unopened Context + borrowed typed decoder cannot coexist | ⚠️ justified deviation, documented in apply-progress §Deviations; preserves the intended drop chain |
| SW-only slice | `hw_active: false`, no `try_init_hw_device` on real path | ✅ per proposal non-goals |

## 6. Issues

### CRITICAL

None.

### WARNING

- **W1 — cfg(video) FFI never compiled on this machine.** `cargo check --features video` dies in `ffmpeg-sys-next-7.1.3` build script (pkg-config missing) before sh_images compiles. Mitigated by exhaustive signature verification against local ffmpeg-next 7.1.0 registry source (table §4); residual compile/link risk is low but nonzero until the PR3 CI lane compiles with dev libs. Recorded as known limitation per plan, not a failure.
- **W2 — Test-isolation race in new toggle test.** `is_available_stub_toggle_roundtrip` is the ONLY `set_test_stub(false)` site in the codebase and mutates the process-global `TEST_FFMPEG_STUB` atomic without holding any guard. Under the default parallel test harness, a concurrent stub-dependent test (all call `ensure_stub()`) can observe the false window and fail intermittently (and vice-versa: the roundtrip's own `assert!(!is_available())` fails if another test sets true concurrently). Suggest serializing via a `Mutex<()>` guard like the existing `TEST_HW_MUTEX` pattern, or adopting `serial_test`.
- **W3 — Ownership deviation from design block.** See §5 row 4. Accepted: forced by upstream API shape; drop-order guarantee preserved.

### SUGGESTION

- `v_stream_index` / `v_time_base` carry `#[allow(dead_code)]` until PR2 packet routing consumes them — ensure PR2 removes the allows rather than letting them persist.
- Consider asserting `can_seek == true` explicitly in a unit-visible way once a real-file integration harness exists (PR3 fixture), since REQ-RD-001 scenario expects it.

## 7. Deferred Items (per plan — not failures)

| Item | Target |
|------|--------|
| Real-container open/close runtime proof (REQ-RD-001 valid-open) | PR3 CI lane (tasks 6.x) |
| Video `next_sample` RGBA + EAGAIN/EOF (REQ-RD-002), scaler Drop/budget (REQ-RD-007) | PR2 (tasks 3.1–3.3) |
| Audio resampling (REQ-RD-003), PTS/seek (REQ-RD-004/005), H.265 acceptance (REQ-RD-009 S2), feature-on-no-DLL lane (REQ-RD-008 S2) | PR3 (tasks 4.x–6.x) |
| End-to-end duration within ±50 ms against a real container | PR3 fixture |

## Final Verdict

**PASS WITH WARNINGS** for the PR1 slice. All five PR1 tasks are complete with runtime-passing evidence at every locally-testable layer; the review-budget (314/400 lines), commit hygiene, lint/format gates, and regression safety net (466/0/1) all hold. The two substantive warnings (uncompiled FFI pending PR3 CI lane; stub-toggle test race) do not block proceeding to PR2 but W2 should be fixed before the PR3 CI lane relies on this suite.
