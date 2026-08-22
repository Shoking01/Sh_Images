# video-decode Specification

## Purpose

SW decode H.264/H.265 mp4/mov/mkv via `ffmpeg-next 7` dynamic LGPL into `FrameRing`/`AudioBuffer`/`PlayClock`. Replaces `next_sample()->EndOfStream` stub; no `Decoder` change; SW only; no VP9/HW.

## Requirements

### Requirement: REQ-RD-001 — Open and error mapping

System SHALL open via `avformat_open_input`+`find_stream_info`, map `AVERROR`→`Media` via `map_av_error`, and before FFI reject missing/empty/corrupt/PNG-masquerade (`validate_file_header`); if `!is_available()` SHALL return `Media` without FFI.

#### Scenario: Valid H.264 opens
- GIVEN valid H.264 mp4 on disk and `is_available()==true`
- WHEN `open(path, output_size, audio_rate)` called
- THEN `Ok` with `width/height` from stream, `fps>0`, `can_seek==true`

#### Scenario: Invalid maps to Media
- GIVEN missing/empty/PNG-masqueraded file or DLLs absent
- WHEN `open` attempted
- THEN `Err(Media)` without panic

### Requirement: REQ-RD-002 — Video next_sample

System SHALL loop `av_read_frame`→`send_packet`/`receive_frame` (`EAGAIN`)→`sws_scale` YUV→RGBA via `SwsContext`→`Sample::Video`; `pts` via `pts_to_duration(time_base)`, `w/h==output_size` cap 1920x1080, `pixels.len()==w*h*4`.

#### Scenario: Produces RGBA
- GIVEN opened file with video packets
- WHEN `next_sample()` called
- THEN `Video` with `len==w*h*4` and `pts>=ZERO`

#### Scenario: Handles EAGAIN/EOF
- GIVEN `receive_frame==EAGAIN`
- WHEN loop continues
- THEN reads next packet; at EOF returns `EndOfStream`

### Requirement: REQ-RD-003 — Audio next_sample

System SHALL decode audio via `SwrContext` to `audio_rate` `FLT` interleaved f32 as `Sample::Audio{pts,samples}`; with `video_only` or no audio stream SHALL never emit `Audio`.

#### Scenario: Resampled audio
- GIVEN file with AAC/MP3, `audio_rate==48000`
- WHEN audio `next_sample` yields
- THEN `samples` interleaved f32 non-empty, `pts` rescaled

#### Scenario: Video-only suppresses audio
- GIVEN `open_video_only`
- WHEN decoding
- THEN no `Audio` variant returned

### Requirement: REQ-RD-004 — PTS monotonicity

System SHALL compute PTS via `pts_to_duration(pts,time_base,fallback,frame_duration)`, fallback `AV_NOPTS_VALUE`→`best_effort_timestamp`, saturate overflow via `i128`, clamp negative→`ZERO`.

#### Scenario: NOPTS fallback
- GIVEN `pts==AV_NOPTS_VALUE`, `best_effort==45000`, `tb 1/90000`
- WHEN `pts_to_duration` called
- THEN `500ms` not `ZERO`

#### Scenario: Overflow saturates
- GIVEN `pts==i64::MAX`, `tb 1/1`
- WHEN `pts_to_duration` called
- THEN large `Duration` without panic

### Requirement: REQ-RD-005 — Seek

System SHALL `av_seek_frame(BACKWARD, av_rescale_q_saturating(target))`+`avcodec_flush_buffers` (video+audio); caller discards `pts<target` pre-roll; rapid seeks coalesce to last target (120ms debounce).

#### Scenario: Seek lands at/behind target
- GIVEN `seek(10s)` from `5s`
- WHEN decoding resumes
- THEN first `pts >=10s` after discarding pre-roll, no stale audio

#### Scenario: Coalesced seeks
- GIVEN 5× `seek_forward 5s` within 120ms
- WHEN flushed
- THEN single `av_seek_frame` with `25s`

### Requirement: REQ-RD-006 — Duration and fps

System SHALL report `duration` from `AVFormatContext::duration` (`AV_TIME_BASE_Q`) or stream duration; `refresh_duration()` re-queries after index load; `fps` from `avg_frame_rate`.

#### Scenario: Duration known
- GIVEN file duration 12.3s
- WHEN `info().duration` or `refresh_duration()` queried
- THEN `Some` within 50ms of true value

#### Scenario: Unknown polled
- GIVEN `duration==AV_NOPTS_VALUE`
- WHEN polled every 30 frames
- THEN `Some` or `None` without panic

### Requirement: REQ-RD-007 — Cleanup and budget

System SHALL own handles via `ffmpeg_next` RAII; `Drop` frees `SwsContext`/`SwrContext`->`AVCodecContext`->`AVFormatContext`, unrefs `AVFrame`/`AVPacket`; `FrameRing` ≤64MB at 1080p (6 slots), full blocks.

#### Scenario: Drop reclaims memory
- GIVEN decoder decoded 60 frames then dropped
- WHEN memory checked
- THEN no native leak, `Drop` without panic

#### Scenario: Budget enforced
- GIVEN 1080p30 decoding
- WHEN ring full
- THEN `capacity_bytes(1920,1080) <=64MB` and decoder waits

### Requirement: REQ-RD-008 — Availability / optional feature

System SHALL keep `video` optional (`default=[]`); `cargo test` without `video` SHALL pass; decode gated by `is_available()` probing `avcodec-61|60|62.dll`; missing DLLs→`Media("FFmpeg DLLs missing")`.

#### Scenario: No feature still tests
- GIVEN `cargo test` without `--features video`
- WHEN tests run
- THEN all pass via stub/error path, no FFI

#### Scenario: No DLLs → Media
- GIVEN `--features video` but no DLLs, no `SH_IMAGES_FFMPEG_STUB`
- WHEN `open` called
- THEN `Media` and `is_available()==false`

### Requirement: REQ-RD-009 — Scope SW-only

System SHALL support only H.264/H.265 mp4/mov/mkv SW; SHALL reject VP9/webm via `check_codec_supported`→`Media`; HW inert unless `hwaccel` enabled.

#### Scenario: VP9 rejected
- GIVEN `.webm` hinting `vp9`
- WHEN `open` validates
- THEN `Err(Media)` mentioning `vp9`

#### Scenario: H.265 accepted
- GIVEN valid H.265 mp4
- WHEN opened SW
- THEN succeeds and decodes like H.264
