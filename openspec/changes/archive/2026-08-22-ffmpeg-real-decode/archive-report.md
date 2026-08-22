# Archive Report: ffmpeg-real-decode

**Archived**: 2026-08-22
**Mode**: openspec (file-based artifacts)
**Archived to**: `openspec/changes/archive/2026-08-22-ffmpeg-real-decode/`
**Final code state**: `main` @ `8a7b9fe` (merge of PR #15)
**Classification**: COMPLETE — intentional-with-warnings solely for the archive-time task reconciliation documented below; no functional gap.

## Change Summary

Replaced the `FfmpegDecoder` stub (`next_sample()` always returning `EndOfStream` → black screen on valid H.264) with real FFmpeg software decode via `ffmpeg-next 7`, feature-gated behind optional `video` (`default=[]`, no `static`). Delivered: real open/close (`avformat_open_input` + `find_stream_info`, owned RAII handles), video `next_sample` (`av_read_frame` → `send_packet`/`receive_frame` → `sws_scale` YUV→RGBA into `Sample::Video`), audio `next_sample` (`SwrContext` → interleaved f32 stereo at device rate), PTS fallback chain (NOPTS → best-effort → saturate/clamp), duration/fps reporting + re-query, seek BACKWARD + dual-decoder flush, drop order scaler/resampler → codec contexts → demuxer, `FrameRing` 64 MB / `AudioBuffer` / `PlayClock` budgets preserved, `Decoder` trait and player protocol unchanged. Minimal ffmpeg-next feature set (codec/format/software-resampling/software-scaling only). SW-only scope: VP9 rejected pre-open; hwaccel remains inert stub behind `hwaccel` feature.

## Delivery

- **3 stacked-to-main PRs**, all MERGED:
  - PR #13 — `8c50ce3` — open/duration slice (tasks 1.1–2.3), 314 changed lines.
  - PR #14 — `dd7cbb4` — video sws decode slice (W2 fix + tasks 3.1–3.3), 302 changed lines.
  - PR #15 — `38b2420` (+ follow-ups) — audio/seek/ci final slice (tasks 4.1–6.3 + W4 disposition), **511 authored lines — `size:exception` accepted by maintainer decision** (excess was test coverage required by the Strict-TDD gate plus the assigned CI lane/docs).
- **8 post-review fix commits** via PR #15 (review-driven corrections after first real compile/run of gated code): `1974f60` (trim ffmpeg-next features — default set needed libavfilter.pc), `49fa855` (scaling import path + usize casts), `920dbdf` (closure under cfg(video)), `510a967` (cargo fmt), `d5c5f0e` (clippy manual_map/manual_clamp under cfg(video)), `b42db77` (demuxer rewind after `open_real`; env-conflict test skip under `SH_IMAGES_FFMPEG_REAL=1`; poison-tolerant mutex), `c26e4ea` (real-decode tests isolated into `tests/video_decode.rs` own integration target), `34eecb3` (Decoder trait import in that target). What/why per commit: see the "Post-review CI fixes" addendum in `apply-progress.md`.
- Branches deleted (local+remote); only `main` carries this work.

## Verification Evidence (final state)

Ranked per Final-State Authority; intermediate snapshots are attributed, never restated as current facts.

1. **Canonical verify-report** (`verify-report.md`): verdict **PASS**, natively admitted by `gentle-ai sdd-verify-validate` (`valid=true`, evidence_revision `sha256:a546a96d…cbda5e`) — **9/9 requirements, 18/18 scenarios**, 0 blockers, 0 critical findings. Gates at its HEAD (`38b2420`): 472/0/1 lib tests, clippy `-D warnings`, fmt exit 0.
2. **CI (terminal receipt of runtime behavior)**: run `32585174001` FULLY GREEN on the merge commit AND on the post-merge main push — all 4 jobs pass, including `video (--features video)` which compiles the FFI against dev libs and RUNS the real-decode suite (`tests/video_decode.rs`, 4 passed). `[ffmpeg-real]` trace confirms packets routed and frames decoded. This converts the canonical report's 8 COMPLIANT-BY-DESIGN scenarios (which deferred end-to-end execution to this lane by plan) into runtime-proven.
3. **Local main @ `8a7b9fe`**: `cargo test --lib` 472 passed / 0 failed / 1 ignored. Independently re-verified at archive time on this HEAD: `cargo fmt --check` exit 0.
4. **Intermediate snapshots** (`verify-report-pr1.md`, `verify-report-pr2.md`): retained for audit history only. At their writing times they were `pass_with_warnings` (PR1 natively denied full-change admission by design, as a stacked slice cannot honestly claim 9/18 while later slices pend). Their pending claims (e.g. "runtime proof pending first CI run", "cfg(video) never compiled") are SUPERSEDED — do not read them as current state.

### W-items final status

| Item | Origin | Final status |
|------|--------|--------------|
| W1 cfg(video) never compiled/run locally | PR1 | **CLOSED WITH RUNTIME PROOF** — CI `video-decode` job compiled and executed real decode (run `32585174001`, `[ffmpeg-real]` trace). |
| W2 stub-toggle race | PR1 | **FIXED + VERIFIED** (PR2): `TEST_HW_MUTEX` guard around toggle sequence. |
| W3 ownership deviation (opened typed decoder vs unopened Context) | PR1 | **CLOSED** — accepted with documented rationale; drop-chain guarantee preserved. |
| W4 ignored `send_eof()` results | PR2 | **DOCUMENTED** — inline deliberate-ignore rationale at both sites; termination argument stated. |

## Review Gate Note

Structured status for this archive contained no `reviewGate` key (receipt-driven development not active for this candidate) → archive proceeded under ordinary repository policy. The runtime ledger's pending `reset` action belongs to the orchestrator and is deliberately NOT executed by this phase.

## Task Completion Gate disposition

Task **7.1** ("fmt FrameRing", cosmetic cleanup) was left unchecked by apply (excluded from the assigned slice scope). It was **reconciled to complete at archive time** under explicit orchestrator closure instruction, with proof:

- Its acceptance criterion ("clippy clean") is proven: CI run `32585174001` green with clippy `-D warnings` on all targets INCLUDING the `--features video` lane; lint fixes landed post-review (`d5c5f0e`, `510a967`).
- Repo-wide `cargo fmt --check` exit 0 re-verified at archive time on `8a7b9fe`.
- `src/core/video/ring.rs` was untouched by this entire change (git log), so the anticipated formatting churn never existed — there is no remaining work the checkbox could represent.
- Reconciliation reason recorded here and inline in `tasks.md`; no code change was involved. The archived tasks artifact therefore shows 17/17 with no stale checkboxes.

## Known limitations / follow-ups

- **H.265 helper-level only**: `check_codec_supported` accepts hevc/h265 and the path is identical to H.264, but no HEVC fixture exists → REQ-RD-009 S2 lacks true end-to-end coverage. Follow-up: tiny HEVC mp4 fixture + gated test.
- **hwaccel still a stub** behind the `hwaccel` feature (SW-only scope per proposal); real HW init (`av_hwdevice_ctx_create`) is future work.
- **MSI DLL bundling needs `cargo wix` validation**: Files.wxs gates exist (DLL presence + EmbedCab + LGPL license) but no installer build ran in CI; validate on a machine with real DLLs before next release cut.
- **Bench baseline first-run pending**: compare `sws_scale` vs pure `yuv420p_to_rgba` oracle in `benches/video_timeline`.
- Task 7.1 itself has no residual work (see reconciliation above).

## Rollback

- Full revert: `git revert` the merge range `607c1e0^..8a7b9fe` (or revert individual commits listed above; slices are separable — infra/docs/fixture commits can be dropped without touching the decoder).
- Partial: reverting `src/core/video/ffmpeg.rs` alone restores stub-only behavior (feature stays optional; default build unaffected).
- NOTE: restoring the old Media Foundation backend is NO LONGER POSSIBLE — `mf.rs` was deleted in the previous archived change (`ffmpeg-video-backend`); FFmpeg is the only backend now.

## Artifact inventory (archived)

| Artifact | Path |
|----------|------|
| Proposal | `proposal.md` |
| Exploration | `explore.md` |
| Delta/full spec | `specs/video-decode/spec.md` |
| Design | `design.md` |
| Tasks | `tasks.md` (17/17 after reconciliation) |
| Apply progress | `apply-progress.md` (3 slice sections + post-review addendum) |
| Verify (canonical) | `verify-report.md` |
| Verify slices | `verify-report-pr1.md`, `verify-report-pr2.md` |
| Archive report | `archive-report.md` (this file) |

Main spec synced at archive: delta copied mechanically (shell copy + byte-identity verification) to `openspec/specs/video-decode/spec.md` (main spec did not previously exist; the delta was a full standalone spec).
