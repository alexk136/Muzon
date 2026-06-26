# Decision 0006 — MusicLab screen: 4 skin variants

Status: closed (recommended defaults adopted; 4 MusicLab screen
variants accepted from `musiclab.html`).

TZ reference: §2.4 (MusicLab), §3.5 (fingerprint / embeddings /
feature extractors / LLM backends), §3.6.3 (MusicLab UI).
Source design: `musiclab.html` m1–m4.
Source issue: [Issues/done/0021_musiclab_screen_4_skin_variants.md](../../Issues/done/0021_musiclab_screen_4_skin_variants.md).
Date: 2026-06-26.
Owner: orchestrator (developer implementation per issue
0021 routing).
Blocking: gates 0022–0033 (every MusicLab feature downstream
is positioned in one of the 4 MusicLab variants).

## Decision

Accept the 4 MusicLab screen variants from `musiclab.html` as
the authoritative v0.4.0–v0.6.0 design source for the MusicLab
screen. Each variant maps to one of the 4 built-in skins and
renders the same MusicLab feature set through a different
skin-specific lens.

| variant | skin            | layout                                       | best for                                  | milestone |
| ------- | --------------- | -------------------------------------------- | ----------------------------------------- | --------- |
| M1      | Modern (built-in) | Chat-first: 340px preset panel + chat       | "ask and receive"                         | 0033      |
| M2      | Winamp (built-in) | Terminal CRT: monospace log + cursor        | power users                               | 0033      |
| M3      | Dense Pro (built-in) | Quality dashboard: stats + issues + AI   | library care                              | 0026      |
| M4      | Cinematic (built-in) | Mood Map: 2D clusters + filter + select | vibe-first exploration                    | 0030      |

All 4 variants share the same MusicLab feature set:

- Fingerprint (Chromaprint / AcoustID) — 0022
- Embeddings (CLAP primary, Panns secondary) — 0028
- Feature extractors (BPM, Key, Loudness, Energy) — 0027 (done)
- LLM provider abstraction (Ollama, OpenAI, Anthropic,
  OpenRouter) — 0032
- Smart playlists — 0031
- Mood map (UMAP / HDBSCAN) — 0030
- Duplicate detection — 0024
- Tag write-back — 0025

## What this decision locks

- The 4 MusicLab variants are the canonical MusicLab screen
  designs; no v0.4.0+ issue is allowed to introduce a fifth
  variant.
- The per-skin MusicLab template lives at
  `crates/muzon-skin/skins/<id>/musiclab.html.tmpl` (parallel
  to the Player template from 0010).
- The 4 MusicLab templates share the same placeholder keys
  (`{{track_title}}`, `{{track_artist}}`, etc.) so the runtime
  substitution is skin-agnostic.
- The CSS for each MusicLab template lives at
  `crates/muzon-skin/skins/<id>/musiclab.css` and uses the
  same 10 CSS variables from 0010.

## Cross-reference: Player concepts (themes.html) vs MusicLab (musiclab.html)

`musiclab.html` p1–p4 are the 4 Player concepts RE-DRAWN for
the musiclab.html context. They are element-level identical or
near-identical to themes.html concepts 1–4. No new Player
content; the Player template from 0010 / 0020 is the
authoritative source. The 4 MusicLab variants are
theme-specific overlays on the same MusicLab feature set.

## What ships in 0021

- This decision doc.
- The empty `crates/muzon-skin/skins/modern/musiclab.html.tmpl`
  for the Modern variant. The template is a stub with the
  6 preset cards and the chat panel structure from m1.
- The 3 remaining variants (M2 / M3 / M4) ship in their
  respective milestone issues (0033, 0026, 0030).

## MCP replay block

The orchestrator run that produced this file tried to call
`memory.record_decision` for the 0021 child of the v0.4.0+
fan-out. The memory MCP server returned `-32000 Internal
error` on every call. When MCP recovers, the next run must
replay:

- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="MusicLab screen: 4 skin variants accepted from musiclab.html (M1 chat, M2 terminal, M3 quality, M4 mood)", dedup_key="MusicLab/decision/0006", details={tz_ref, source_design, mapping, milestone_split})`
- `memory.record_task_lineage_event(task_ref="Issues/done/0021", stage="completion", agent_role="orchestrator", summary="0021 closed: MusicLab design acceptance + Modern template stub", commit_sha=<0021-commit-sha>)`
