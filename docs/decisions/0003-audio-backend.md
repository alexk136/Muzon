# Decision 0003 — Audio backend: GStreamer `playbin3` + `appsink` (v0.1.0–v1.0.0)

Status: closed (recommended default adopted; libpipewire punted to post-v1.0.0).
TZ reference: §2.1.1, §3.2, §6, §7.
Source issue: [Issues/done/0003_finalize_audio_backend_and_bit_perfect_path.md](../../Issues/done/0003_finalize_audio_backend_and_bit_perfect_path.md).
Date: 2026-06-26.
Owner: orchestrator (developer spike owner per issue 0003 routing).
Blocking: hard prerequisite for `Issues/open/0009_minimal_cli_muzon_play_via_gstreamer.md`.

## Decision

Adopt **GStreamer** as the only audio backend in v1.0.0, using
`playbin3` + `appsink` as the canonical pipeline shape, with an
explicit "bit-perfect" user toggle that strips the resampler from
the user-visible chain. The optional `libpipewire` direct path from
TZ §3.2 is **punted to post-v1.0.0**; the §3.2 "опционально"
language is permission to defer, not a commitment to ship.

The four audio features that drive the choice (TZ §2.1.1) are all
achievable on the GStreamer path:

- **Gapless playback**: `playbin3`'s `about-to-finish` signal is the
  documented mechanism; TZ §3.2 notes GStreamer has a `gapless`
  element but it is finicky on `playbin` transitions, hence
  `playbin3` rather than `playbin`.
- **ReplayGain**: a custom `audioconvert` chain with `rgvolume` (or
  a clean `playbin3` knob, depending on what the spike confirms).
- **Crossfade**: manual `pad` handoff between two `playbin3`
  instances with synchronized clocks.
- **Bit-perfect**: GStreamer with `alsasink` + the right `audio/x-raw`
  caps and no `audioconvert` in the chain. Verified with `pw-cli` or
  `alsacap` that the actual output sample rate and bit depth match
  the source.

The visualization PCM feed for the §2.1.3 spectrum is the same
`appsink` callback that drives the FFT, which keeps the visualization
free of any extra DSP path.

## What this decision locks for v0.1.0+

- The `muzon-audio` crate (TZ §3.1.1) uses GStreamer via
  `gstreamer-rs`.
- The pipeline shape is `playbin3` + `appsink`. v0.1.0 only needs
  the `playbin3` part; the `appsink` PCM tap is added in v0.2.0
  when the spectrum UI lands.
- The "bit-perfect" toggle is a runtime config flag
  (`audio.bit_perfect`, default `false`) read by 0006's config layer.
- v0.1.0 CLI prototype (`Issues/open/0009_minimal_cli_muzon_play_via_gstreamer.md`)
  uses `playbin3` for `muzon play <file>`; the spike is a v0.2.0+
  follow-up to confirm the bit-perfect path with `pw-cli` /
  `alsacap`.

## Why not ship libpipewire in v1.0.0

Maintaining two parallel playback engines doubles the cost of every
new feature (gapless, ReplayGain, crossfade, seek, mini-mode A/V
sync, etc.). The §3.2 "опционально" line is the only commitment in
TZ; §6 lists GStreamer as a known risk (medium probability, medium
impact) and the bit-perfect promise in §2.1.1 is implementable on
the GStreamer path. Deferring libpipewire does not break any TZ
feature.

If a user actually needs libpipewire-level bit-perfect, the
recommended path is to use the system PipeWire session manager
(which is what `playbin3` + `pipewiresink` already targets) rather
than a parallel engine. Revisit libpipewire direct binding only if
post-v1.0.0 telemetry / community feedback shows a real demand that
the GStreamer path cannot meet.

## Spike scope (deferred to v0.2.0)

The 1-2 week spike from issue 0003 is explicitly **out of scope for
v0.1.0**: v0.1.0 only needs `playbin3` to play a single file
end-to-end (issue 0009). The spike gates v0.2.0+ audio work and must
produce:

1. **Pipeline-shape confirmation.** Documented `playbin3` flags and
   `appsink` caps string. Confirmed: FLAC, MP3, OGG/Vorbis, Opus
   play to completion on a Fedora + Ubuntu reference machine.
   AAC/M4A, WAV, APE, WMA confirmed if the relevant `gst-plugins`
   are present in the default distro install; deferred if not.
2. **Gapless + ReplayGain + crossfade POC.** A 30-line Rust program
   that:
   - Plays two FLAC files gaplessly (continuous sine-wave test
     sample).
   - Applies track-mode ReplayGain on a -23 LUFS reference and
     asserts the output dB delta.
   - Crossfades 3 s between the two and asserts no glitch.
   - Records CPU and memory during playback.
3. **Bit-perfect path.** A second program (or a flag on the first)
   that opens an ALSA device, plays a 24-bit/96 kHz FLAC, and uses
   `pw-cli` or `alsacap` to verify the sink format matches the
   source. The same program uses the PipeWire `pw-stream` path on
   a PipeWire session; the spike documents whether the same code
   targets both or if a feature flag is required.

If the spike confirms §2.1.1, the v0.2.0 audio module ships as
specified. If the spike finds that gapless or bit-perfect is
unachievable on `playbin3`, the spike recommends a concrete
fallback (custom `playbin3` configuration, or a return to
`playbin` with manual `about-to-finish` handling) and the v0.2.0
milestone absorbs the rework.

## Flatpak GStreamer plugin bundling

TZ §3.8 names Flatpak as the primary distribution. The GStreamer
plugins that ship with the Flatpak are: `gst-plugins-base`,
`gst-plugins-good`, `gst-plugins-bad` (FLAC, MP3, AAC, Opus are all
in `base` or `good`; APE and WMA are in `bad`; ALAC is in `bad`).
The Flatpak manifest in v1.0.0 packaging work bundles the
necessary subset. The v0.2.0 spike records any plugin that the
reference distro install has but the Flatpak does not, so the v1.0.0
packaging work has a concrete list.

## MCP replay block

The orchestrator run that produced this file tried to call
`memory.record_decision` and `memory.record_task_lineage_event` for
the 0003 child of the umbrella. The memory MCP server returned
`-32000 Internal error` on every call. When MCP recovers, the next
run must replay:

- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="Audio backend: GStreamer playbin3+appsink; libpipewire punted to post-v1.0", dedup_key="MusicLab/decision/0003", details={tz_ref, source_issue_ref, locked_for=["v0.1.0","v0.2.0","v0.3.0","v0.4.0","v0.5.0","v0.6.0","v1.0.0"], deferred=["libpipewire-direct-path"], blocks=["0009"]})`
- `memory.record_task_lineage_event(task_ref="Issues/done/0003", stage="completion", agent_role="orchestrator", summary="0003 closed: GStreamer audio backend (libpipewire deferred)", commit_sha=<0003-commit-sha>)`
