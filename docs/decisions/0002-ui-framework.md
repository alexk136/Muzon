# Decision 0002 — UI framework: Tauri 2.x (provisional)

Status: provisional decision adopted; prototype spike deferred to v0.2.0 kickoff.
TZ reference: §3.6, §6, §7 question 3.
Source issue: [Issues/done/0002_finalize_ui_framework_tauri_vs_slint.md](../../Issues/done/0002_finalize_ui_framework_tauri_vs_slint.md).
Date: 2026-06-26.
Owner: orchestrator (developer spike owner per issue 0002 routing).

## Decision

Adopt **Tauri 2.x** as the UI framework for v0.2.0 and forward, with
**Slint** as the documented fallback if the spike finds concrete
WebView breakage on two or more of the four target distros (Fedora,
Ubuntu, Arch, NixOS). `iced` is removed from the shortlist because it
shares Slint's "skin format becomes a custom renderer" cost without a
materially different tradeoff.

## Decision matrix (orchestrator-filled)

| Axis | Tauri 2.x | Slint | Winner |
| --- | --- | --- | --- |
| WebView coverage on Fedora/Ubuntu/Arch/NixOS | weak spot (§6 risk: WebKitGTK variance; NixOS needs explicit flags) | native, predictable on every distro | Slint (potential) |
| §2.3 layered-skin implementation cost | "few dozen lines" of HTML/CSS, CSS variables for theming (TZ §3.3) | custom Slint renderer OR Skia backend, non-trivial | Tauri (decisive) |
| Bundle size | WebView + frontend bundle, larger than a pure Rust UI binary | single binary, smaller | Slint |
| Cold start | WebView warm-up, depends on distro | instant | Slint |
| Idle RAM | 60–150 MB typical for a Tauri/WebKitGTK shell, plus frontend | lower | Slint |
| §3.7 perf budget (60 fps UI, < 200 MB idle, < 500 MB with MusicLab map) | achievable; frontend hot path runs in WebView | achievable; everything is native | tied |
| Recruitability (Rust UI devs) | TS/React background transfers; large web hiring pool | smaller pool, more native-Rust-aligned | Tauri |
| Long-term maintainability | Tauri 2.x release cadence is active in 2026, single-org risk (Tauri Foundation) | Slint has commercial and dual-licensing story; community edition is GPL-compatible, commercial license for closed use | Slint (slight) |
| Frontend stack compatibility (§3.6.1: TypeScript + React, Zustand/RTK, radix-ui) | direct fit | full replacement required | Tauri |

Net: Tauri wins on the two axes that most affect product
differentiation (skin format, recruitability). Slint wins on the two
axes that affect packaging and stability (distro coverage, bundle
size). The §6 "Tauri on Linux is unstable" risk is real but is
explicitly listed as low-probability, high-impact; the §6 "skin
quality is spaghetti" risk is high-probability, medium-impact, and
tilts the matrix toward Tauri.

## Fallback trigger

Switch to Slint if the v0.2.0 spike (see "Spike scope" below) finds
**concrete WebView breakage on two or more of the four target
distros**. "Concrete" means reproducible build or runtime failure,
not "slightly different behavior". If only one distro is broken, fix
forward in Tauri and document the workaround in the Flatpak manifest.

## Spike scope (deferred to v0.2.0)

The issue 0002 spike is a 1-2 week, two-track effort. It is
explicitly **out of scope for v0.1.0**: v0.1.0 has no UI
(`Issues/open/0005_cargo_workspace_skeleton_and_ci.md` and
`Issues/open/0009_minimal_cli_muzon_play_via_gstreamer.md` are CLI
only). The spike gates v0.2.0 "MVP Player" and must produce the
following artifacts before v0.2.0 starts:

1. **Track A — Tauri 2.x prototype.** A minimal "now-playing screen
   with one skin and a play/pause button that flips a `tracing::info!`
   line". Built on Fedora, Ubuntu, Arch, NixOS (Docker or VM harness).
   Recorded: bundle size, cold start, idle RAM, WebView version on
   each distro.
2. **Track B — Slint prototype.** Same screen, equivalent controls.
   Same four distros. Same measurements. Additional: a "layered skin
   POC" that swaps the background PNG at runtime, with the LOC and
   complexity recorded.
3. **Decision (≤ 1 page).** Filled-in matrix above, plus the
   recording from Track A and Track B, plus a one-line
   "stay / switch" recommendation. Submitted to orchestrator.

If Track A is green on three or more distros and Track B's layered
skin POC costs more than 200 LOC, stay with Tauri. If Track A is red
on two or more distros, switch to Slint. Otherwise orchestrator
arbitrates.

## What this decision locks for v0.2.0

- The `muzon-ui` crate (TZ §3.1.1) uses Tauri 2.x.
- The frontend stack is TypeScript + React + Zustand (or RTK) +
  radix-ui primitives, per TZ §3.6.1.
- Skin format is HTML + CSS variables, layered (background / main /
  buttons / spectrum), PNG and SVG, per TZ §2.3 and §3.3.
- The "no telemetry" decision from
  [DECISIONS.md 0001-N4](DECISIONS.md#decision-0001-n4--telemetry-no-telemetry-ever)
  applies to the Tauri webview the same way it applies to the Rust
  binary: the release build makes zero outbound network calls
  without an explicit opt-in.

## Non-goals for this decision

- React vs Svelte (frontend framework) — v0.2.0 follow-up.
- Tailwind vs vanilla CSS (theming library) — v0.2.0 follow-up.
- Tauri-specific IPC vs Unix-socket IPC (TZ §3.1.2) — closed in
  0005/0009.
- `iced` — removed from the shortlist; revisit only if Slint and
  Tauri both fail.

## MCP replay block

The orchestrator run that produced this file tried to call
`memory.record_decision` and `memory.record_task_lineage_event` for
the 0002 child of the umbrella. The memory MCP server returned
`-32000 Internal error` on every call. When MCP recovers, the next
run must replay:

- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="UI framework: Tauri 2.x (provisional, Slint fallback)", dedup_key="MusicLab/decision/0002", details={tz_ref, source_issue_ref, matrix, fallback_trigger, spike_scope})`
- `memory.record_task_lineage_event(task_ref="Issues/done/0002", stage="completion", agent_role="orchestrator", summary="0002 closed: Tauri-vs-Slint spike (provisional Tauri decision, prototype deferred to v0.2.0)", commit_sha=<0002-commit-sha>)`
