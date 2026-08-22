```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:e553b684789537f854a0445286964493e203500242600827d4e18cef55edbb80
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 1/9
scenarios: 5/18
test_command: cargo test --lib
test_exit_code: 0
test_output_hash: sha256:156a500634277a5e5e93ad9404ebc81f0c7d647c12f7c1005ebe3a3652f4dc44
build_command: cargo clippy --all-targets -- -D warnings
build_exit_code: 0
build_output_hash: sha256:0546b0e232ba97124afe947d76986efe310b8a8b7ec8f0e12448e9b980bafe5f
```

# Verify Report — ffmpeg-real-decode PR2 (video decode slice)

> **Admission note (recorded, per verify contract).** Candidate bytes validated with `gentle-ai sdd-verify-validate --input <candidate> --requirements 9 --scenarios 18` before any write; see §8 for the exact validator outcome. Per explicit orchestrator instruction this PR2 report is persisted as a **slice artifact** (`verify-report-pr2.md`); the canonical `openspec/changes/ffmpeg-real-decode/verify-report.md` remains unwritten and MUST pass native admission at final verification after the last slice lands.

- **Change**: `ffmpeg-real-decode` — PR2 slice only (W2 fix + tasks 3.1, 3.2, 3.3)
- **Mode**: openspec (repo-local)
- **Branch / commit**: `feat/ffmpeg-real-decode-pr1` @ `dd7cbb4` (`feat(video): decode video frames via SwsContext into Sample::Video`), stacked on PR1 `8c50ce3`
- **Scope note**: tasks 4.x–7.x belong to PR3; they are intentionally unchecked in this stacked chain and are NOT failures of this verification. Audio REQ-RD-003 is out of PR2 scope per plan.
- **Evidence revision**: sha256 of the reviewed patch bytes (`git diff 8c50ce3..dd7cbb4 -- src/core/video/ffmpeg.rs`) = `sha256:e553b684…edbb80`; HEAD = `dd7cbb43000cc4ac0a34be15eb0f9f8aca2807fc`.

## 1. Task Completeness

| Task | State | Evidence |
|------|-------|----------|
| W2 fix (verify-report-pr1) | ✅ done | Single `set_test_stub(false)` site (ffmpeg.rs:1094) inside `is_available_stub_toggle_roundtrip`, now holds `TEST_HW_MUTEX.lock()` (line 1088) for the whole toggle sequence; repo-wide grep confirms **no other unguarded site**; focused 1/1 + full suite green |
| 3.1 [RED] RGBA `len==w*h*4` | ✅ done | Pure `copy_rgba_rows` + 3 runtime-green unit tests (padded stride / tight stride / truncated-no-panic); compile-gated integration test `next_sample_real_decode_pixels_len_equals_wh4` asserts the REQ-RD-002 invariant once DLLs+fixture exist (`#[cfg(feature="video")] #[ignore]`, early-returns without them) |
| 3.2 `next_sample` av_read→SwsContext→Video | ✅ done | Real loop behind `#[cfg(feature="video")]`: reusable `Packet::empty()` + `packet.read(&mut ictx)` → stream routing → `send_packet` → `receive_frame`; EAGAIN/Eof/error arms per design; scaler rebuild on definition change; row-wise RGBA extraction; pts via `pts_to_duration`. Compile-gate only on this machine (see W1-carried); all call sites registry-verified 4/4 (§4) |
| 3.3 `Drop` scaler→ctx→ictx | ✅ done (structural) | `RealOpen` field order `scaler → v_out → v_decoder → … → ictx`; Rust drops fields in declaration order ⇒ sws freed first, demuxer last (REQ-RD-007). Leak/budget runtime proof stays deferred (PR1 §7) |

PR2 slice: **3/3 tasks + W2 fix complete**. Whole change: 8/16 tasks (PR3 pending).

## 2. Runtime Evidence

| Command | Result | Exit |
|---------|--------|------|
| `cargo test --lib` | 469 passed / 0 failed / 1 ignored (+3 executable vs PR1, zero regressions) | 0 |
| `cargo test --lib core::video::ffmpeg` | 46 passed (43 prior + 3 new pure tests) | 0 |
| `cargo clippy --all-targets -- -D warnings` | clean, Finished | 0 |
| `cargo fmt --check` | clean | 0 |
| `git log --oneline -3` | two work-unit commits stacked: `dd7cbb4` (PR2) atop `8c50ce3` (PR1) atop `02a1524` | 0 |
| `git diff 8c50ce3..dd7cbb4 --stat` | 1 file, `src/core/video/ffmpeg.rs` +291/−11 = **302 lines ≤ 400 budget** | 0 |
| `cargo check --features video` | FAILED at dependency build script: `ffmpeg-sys-next v7.1.3` build-script-build exit 101, panic at `ffmpeg-sys-next-7.1.3\build.rs:1035:14` — "The pkg-config command could not be found" (libavutil). Dies BEFORE any sh_images code compiles (no `Checking sh_images` reached). Same known machine limitation as PR1 W1; recorded as deferred item, NOT a failure. | 101 |

Evidence output hashes (canonical bytes under `%TEMP%\opencode\`): test `sha256:156a5006…f4dc44`, clippy `sha256:0546b0e2…80bafe5f`.

## 3. Spec Compliance Matrix (authoritative spec totals: 9 requirements / 18 scenarios)

PR2-owned scenarios are REQ-RD-002 (both) and REQ-RD-007 S1; REQ-RD-007 S2 gains decoder-side partial evidence.

| Requirement | Scenario | Status | Covering evidence |
|-------------|----------|--------|-------------------|
| REQ-RD-001 Open/error mapping | Valid H.264 opens | DEFERRED (UNTESTED here) | PR3 CI lane per plan (unchanged from PR1) |
| REQ-RD-001 Open/error mapping | Invalid maps to Media | ✅ PASS | PR1 tests still green in this run (inside 469) |
| REQ-RD-002 Video next_sample | Produces RGBA | ⚠️ IMPLEMENTED / runtime DEFERRED | Invariant decomposition proven at runtime on pure layer: `rgba_rows_len_is_wh4_with_padded_stride`, `rgba_rows_tight_stride`, `rgba_rows_truncated` (len==w·h·4 always, padding stripped, hostile input safe) + `pts_negative_clamps_to_zero_monotonic` (pts≥ZERO clamp). End-to-end proof blocked by missing dev libs/DLLs → compile-gated `next_sample_real_decode_pixels_len_equals_wh4` + PR3 fixture (task 6.1) |
| REQ-RD-002 Video next_sample | Handles EAGAIN/EOF | ⚠️ IMPLEMENTED / runtime DEFERRED | Loop arms code-reviewed line-by-line against registry semantics (EAGAIN=`Error::Other{errno:EAGAIN}` verified round-trip in ffmpeg-next source error.rs:370; EOF=`Error::Eof`; packet-EOF→`send_eof`+`draining` flag prevents double-flush; drain+EAGAIN→EndOfStream). No runnable harness on this machine |
| REQ-RD-003 Audio next_sample | both | OUT OF SLICE (PR3) | Non-video packets consumed-and-skipped is the documented PR2 placeholder |
| REQ-RD-004 PTS monotonicity | both | PARTIAL (hooks only) | `pts_to_duration(pts \| best_effort, tb)` wired with NOPTS fallback chain (`pts()`→None→`AV_NOPTS_VALUE`→helper→`timestamp()`=best_effort per registry mod.rs:117); negative-clamp covered at runtime. Full NOPTS matrix + frame-duration fallback explicitly deferred to task 5.1 per apply-progress deviation #5 |
| REQ-RD-005 Seek | both | OUT OF SLICE (PR3) | — |
| REQ-RD-006 Duration/fps | Duration known | ✅ PASS (unit layer) | PR1 tests still green |
| REQ-RD-006 Duration/fps | Unknown polled | ✅ PASS (unit layer) | PR1 tests still green |
| REQ-RD-007 Cleanup/budget | Drop reclaims memory | ⚠️ STRUCTURAL PASS / leak proof DEFERRED | Drop ORDER proven structurally (field declaration order); RAII ownership via ffmpeg-next wrappers (no raw `*mut AV*` introduced); 60-frame leak check requires runnable decode → PR3 CI lane |
| REQ-RD-007 Cleanup/budget | Budget enforced | ✅ PASS (ring side) + partial decoder side | `bench_gate_ring_capacity_stays_under_64mb` green in this run; decoder-side v_out capped at 1920×1080×4 ≈ 7.9 MiB/frame (dst dims `min(1920)/min(1080)` enforced in `video_sample`) |
| REQ-RD-008 Availability/feature | No feature still tests | ✅ PASS | This verification literally ran `cargo test --lib` without `video`: 469/0/1, default build links no FFI |
| REQ-RD-008 Availability/feature | No DLLs → Media | DEFERRED (partial) | Needs compilable cfg(video) → PR3 CI lane |
| REQ-RD-009 Scope SW-only | VP9 rejected | ✅ PASS (helper layer) | PR1 test green; rejection ordering untouched by PR2 diff |
| REQ-RD-009 Scope SW-only | H.265 accepted | DEFERRED (UNTESTED) | Needs real decode → PR3 |

**Strict yaml counts unchanged from PR1 (1/9 requirements, 5/18 scenarios)**: no NEW scenario acquired a passing *runtime* covering test in PR2 — every PR2-owned path is compile-gated off on this machine by design; the additions are structural/pure-layer evidence plus the compile-gated integration harness.

## 4. Correctness Review (diff vs spec/design)

| Check | Verdict | Detail |
|-------|---------|--------|
| All new FFI strictly `#[cfg(feature = "video")]` | ✅ | `ScalerDef`, expanded `RealOpen`, `is_av_eagain`, `next_sample_real`, `video_sample`, cfg import of `ring::VideoFrame`, and the `next_sample` dispatch block are individually cfg-gated; `copy_rgba_rows` is intentionally ungated PURE Rust (no FFI) so its tests run in the default build; feature-off `next_sample` falls through to stub `EndOfStream`. Default build compiles & passes (469/0/1 proves no stray symbol) |
| Packet loop: single reusable Packet | ✅ | One `Packet::empty()` outside the loop; reused via `packet.read(&mut real.ictx)` each iteration (registry packet.rs:217 wraps `av_read_frame`). Matches design "Reuse one Packet"; FFmpeg 7 unref-on-read semantics documented in code comment |
| EAGAIN handling via errno | ✅ | `is_av_eagain` matches `Error::Other { errno } if errno == EAGAIN` — registry-verified: AVERROR(EAGAIN)↔`Other{errno:EAGAIN}` round-trip (error.rs:370); receive-EAGAIN → fall through to packet read |
| EOF mapping | ✅ | receive_frame `Err(Eof)` → `Sample::EndOfStream`; packet read `Err(Eof)` → `send_eof()` once (guarded by `draining` flag) → continue receiving buffered frames → drain+EAGAIN or receive-Eof → EndOfStream. Matches spec scenario S2 shape |
| Errors mapped via `map_av_error` with codec name | ✅ | All three FFI failure sites use `map_av_error(i32::from(err), &real.codec_name)`; `From<Error> for c_int` verified (error.rs:113); codec captured pre-open |
| No unwrap/panic on FFI paths | ✅ | Production FFI paths use `map_err` + `let Some(…) else`; `unwrap`s exist only in `#[cfg(test)]` code (mutex guards). Note: upstream `Video::data(i)` panics on out-of-range index, but calls are hard-indexed `stride(0)/data(0)` on RGBA output which always has ≥1 plane after a successful `run()` |
| Scaler once per definition, capped dims | ✅ | `Option<ScalerDef>` in `RealOpen`, lazily created, rebuilt iff src_fmt/src_w/src_h/dst_w/dst_h changed; dst capped `.min(1920).max(1)` / `.min(1080).max(1)`; `InputChanged` prevented-by-construction (context rebuilt before `run` can reject) and documented in ScalerDef docs |
| Row-wise copy strips stride padding | ✅ | `copy_rgba_rows(src, stride, w, h)` copies exactly `w*4` per row from stride-offset rows into a tight `w*h*4` buffer; saturating math + truncation-safe break; 3 unit tests cover padded/tight/truncated |
| pts wired through time_base, NOPTS fallback present | ✅ | `decoded.pts().unwrap_or(AV_NOPTS_VALUE)` + `v_time_base.{numerator,denominator}()` (i32, matches helper) + `timestamp()` as best-effort fallback (registry mod.rs:115-121 reads `best_effort_timestamp`) + `None` frame-duration; result feeds `VideoFrame{pts: Duration,…}` (ring.rs:41 fields match exactly) |
| Drop order scaler→decoder→input | ✅ | Field declaration order `scaler, v_out, v_decoder, …, ictx` ⇒ drop order scaler(+v_out) → Video→Opened→Context → Input last. PR1's `#[allow(dead_code)]` on `v_stream_index`/`v_time_base` removed as PR1 suggested — both now consumed |
| W2 fix scoped correctly | ✅ | Guard added at top of toggle test; repo-wide grep: exactly one `set_test_stub(false)` call site, now guarded; other global-state test (`open_with_hw_force_fail…`) already held its own lock (line 1365) |
| Stub/default behavior preserved | ✅ | Feature-off `next_sample` body identical semantics to PR1 (EndOfStream); player.rs/ring.rs/backend.rs/Cargo.toml untouched by the commit (302-line single-file stat confirms) |

### Registry-source verification spot check (orchestrator item 5) — **4/4 claims CONFIRMED**

Checked under `~\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\ffmpeg-next-7.1.0\src\`:

1. ✅ `scaling::Context::get` — software/scaling/context.rs:36-43: exactly 7 args `(src_format, src_w, src_h, dst_format, dst_w, dst_h, flags)`, wraps `sws_getContext`; `run` :130 takes `(&frame::Video, &mut frame::Video)`; `InputChanged` :135 returned on definition mismatch (validates prevent-by-rebuild strategy).
2. ✅ `Packet::read` — codec/packet/packet.rs:217: `read(&mut self, &mut format::context::Input)` wrapping `av_read_frame`; `empty()` :24; `stream() -> usize` :111; bonus `impl Ref for Packet` :256 makes `send_packet::<P: packet::Ref>(&packet)` type-check (opened.rs:36).
3. ✅ `Error::Other { errno: EAGAIN }` — util/error.rs:70-71 ("For AVERROR(e) wrapping POSIX error codes, e.g. AVERROR(EAGAIN)"), `Eof` :44, `From<Error> for c_int` :113, and the crate's own test :370 asserting `Error::from(AVERROR(EAGAIN)) == Error::Other { errno: EAGAIN }`.
4. ✅ `frame.data(0)` stride semantics — util/frame/video.rs:292: `data(index)` length = `stride(index) * plane_height(index)` (linesize-padded plane, exactly what `copy_rgba_rows` strips); `stride(usize)->usize` :196; `width/height -> u32` :97/:109; Deref/DerefMut to `Frame` :320/:329 (enables method reuse incl. `pts/timestamp`); `Frame::pts()` maps NOPTS→None (mod.rs:98-104) and **`timestamp()` reads `best_effort_timestamp`** (mod.rs:115-121) — confirming the PR2 fallback chain is genuinely best-effort.

## 5. Design Coherence

| Design decision | Implementation | Coherent? |
|-----------------|----------------|-----------|
| One SwsContext per decoder, recreate on renegotiate | `ScalerDef` staleness covers format AND size changes incl. future `set_output_size` (lazy rebuild documented in that method) | ✅ (superset; apply-progress deviation #2) |
| Reuse one Packet + Frames | One reusable `Packet`; decoded frame recreated per call while RGBA `v_out` is reused — documented tradeoff (apply-progress deviation #4) | ⚠️ accepted, documented |
| sws_scale YUV→RGBA via SwsContext | Uniform path through libswscale for ALL formats (no RGBA fast-path) — justified: SW H.264/H.265 never emits RGBA natively | ✅ (apply-progress deviation #3) |
| Task text `scaling::Context::get_format` | Actual API is `Context::get(...)` (7 args, registry-verified); same semantics as sws_getContext | ⚠️ naming-only deviation, documented (#1) |
| Drop: scaler/resampler → ctx → ictx | Declaration-order drop in `RealOpen` | ✅ |
| next_sample loop shape (design pseudo-code) | Implementation matches: read → route by stream index → send → receive → EAGAIN continue / EOF drain; non-video packets consumed-and-skipped until PR3 audio routing | ✅ for PR2 scope |

## 6. Issues

### CRITICAL

None.

### WARNING

- **W1 (carried from PR1) — cfg(video) FFI never compiled on this machine.** `cargo check --features video` panics in `ffmpeg-sys-next-7.1.3\build.rs:1035:14` (pkg-config missing) BEFORE sh_images compiles; exit 101 re-confirmed this session. Mitigation strengthened for PR2: 4/4 API claims independently re-verified against registry source (§4 table). Residual compile/link risk low but nonzero until the PR3 CI lane builds with dev libs. Known limitation per plan — NOT a slice failure.
- **W4 — Drain-mode robustness relies on `let _ = send_eof()` (error ignored).** If `avcodec_send_eof` ever failed, `draining=true` still forces termination at the next EAGAIN, so the loop cannot hang — acceptable best-effort drain — but the ignored Result is silent. Suggest documenting the deliberate ignore (or logging at PR3 when a tracing story exists).

### SUGGESTION

- `send_packet` treats any error (incl. theoretical EAGAIN) as fatal Media. After a receive-EAGAIN the decoder wants input, so send-EAGAIN violates FFmpeg's contract here; fine to leave, worth a comment.
- Integration test could additionally assert `frame.is_consistent()` (ring.rs helper) once runnable — free redundancy for REQ-RD-002.

## 7. W-items Status (orchestrator checklist)

| Item | Origin | Status |
|------|--------|--------|
| W1 cfg(video) uncompiled locally | PR1 report | OPEN (carried) — machine limitation, target PR3 CI lane (tasks 6.x) |
| W2 stub-toggle race | PR1 report | **FIXED & VERIFIED** — mutex guard added; single-site grep proof; suite green |
| W3 ownership deviation | PR1 report | CLOSED (accepted in PR1 with documented rationale) |
| W4 ignored `send_eof()` result | this report | NEW, minor — suggestion-level follow-up for PR3 |

## 8. Validator Admission Outcome

`gentle-ai sdd-verify-validate --input <candidate> --requirements 9 --scenarios 18` was executed against these exact bytes prior to persistence and **denied admission** (exit 1):

```
Error: verify report admission denied: passing verdict contradicts failing or incomplete evidence
```

Root cause is identical to the PR1 denial: the native envelope requires any non-`fail` verdict to carry complete counts (`requirements == total`, `scenarios == total`), which an honest mid-chain slice report cannot assert while 13 scenarios await PR3. Nothing failed inside this slice's scope — tasks 3.1–3.3 + W2 are complete with green gates — so a `fail` verdict would be equally false. Per the explicit slice-verification instruction, these bytes are persisted unchanged as `openspec/changes/ffmpeg-real-decode/verify-report-pr2.md`; the canonical `openspec/changes/ffmpeg-real-decode/verify-report.md` remains unwritten and MUST pass native admission at final verification after PR3 lands.

## Final Verdict

**PASS WITH WARNINGS** for the PR2 slice. Tasks 3.1–3.3 and the W2 fix are complete; every locally-executable gate is green (469/0/1, focused 46/0, clippy clean, fmt clean); review budget holds at 302/400 lines across two stacked work-unit commits; all four ffmpeg-next API claims independently confirmed against registry source. PR2-owned spec scenarios (REQ-RD-002 ×2, REQ-RD-007 S1) are implemented and structurally/pure-layer evidenced, with end-to-end runtime proof deferred BY PLAN to the PR3 CI lane — the same deferral class PR1 recorded, not a regression. Proceed to PR3 (tasks 4.x–6.x).
