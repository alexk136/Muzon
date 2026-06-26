# Decision 0005 — Skin system and default theme

Status: closed (recommended defaults adopted; full HTML/CSS port of
the Modern Skin to `crates/muzon-skin/skins/modern/`; the 3
non-default skins ship as placeholders; full ports are 0017).
TZ reference: §2.3, §3.3, §3.6.2, §6 (skin quality risk).
Source design: `themes.html` (4 design concepts v0.2).
Source issue: [Issues/done/0010_skin_system_4_built_in_default_modern.md](../../Issues/done/0010_skin_system_4_built_in_default_modern.md).
Date: 2026-06-26.
Owner: orchestrator (developer implementation per issue 0010 routing).
Blocking: gates 0011, 0015, 0016, 0017, 0018, 0019, 0020.

## Decision

Adopt the four `themes.html` design concepts as the v0.3.0
built-in skin set, with the following stable ids and display
names:

| id            | display name       | themes.html concept  | grid shape                       |
| ------------- | ------------------ | -------------------- | -------------------------------- |
| `modern`      | Modern Skin        | Concept 1 "Modern Player"  | `topbar-sidebar-main-player` |
| `winamp`      | Winamp Skin        | Concept 2 "Winamp-Inspired" | `titlebar-player-playlist` |
| `dense_pro`   | Dense Pro Skin     | Concept 3 "Dense Pro"       | `top-sidebar-main-right-player` |
| `cinematic`   | Cinematic Skin     | Concept 4 "Cinematic"       | `top-main-player`            |

Default: **Modern Skin** (`modern`). This is the v0.1.0 and
v0.2.0 default; the user can change it in Settings (0019) and
the change persists in `~/.config/muzon/config.toml` as
`ui.default_skin = "<id>"`.

The 4 ids are stable identifiers in code and config; the
display names are user-facing strings (English v1.0 locale).

## Skin manifest schema (`skin.toml`)

```toml
[skin]
id = "modern"
name = "Modern Skin"
version = "0.1.0"
author = "Muzon contributors"
license = "MIT OR Apache-2.0"
min_muzon_version = "0.2.0"

[theme]
mode = "dark"     # dark | light | auto
accent = "#8b5cf6"
palette = "theme.css"

[layout]
main_template = "index.html.tmpl"
mini_template = ""   # empty = derive from main; full mini in 0020
grid = "topbar-sidebar-main-player"   # one of the 4 grid shapes

[layers]
background = ["assets/bg/*"]
main       = ["assets/main/*"]
buttons    = ["assets/buttons/*"]
spectrum   = ["assets/spectrum/*"]
```

Grid shape values for the 4 built-in skins:

- `modern` → `topbar-sidebar-main-player`
- `winamp` → `titlebar-player-playlist`
- `dense_pro` → `top-sidebar-main-right-player`
- `cinematic` → `top-main-player`

The `muzon-skin` validator refuses an unknown `grid` value.
This is the TZ §6 "skin quality is spaghetti" mitigation: a
skin cannot redefine the grid, only the assets and the
CSS variables. The grid contract is the floor; everything
above it is the skin's responsibility.

## CSS-variable contract (from `themes.html`)

Every built-in skin **must** define the following 10 CSS
custom properties at the top of its `theme.css`:

- `--bg-0`, `--bg-1`, `--bg-2`, `--bg-3` (background ramp)
- `--border` (one-pixel separator colour)
- `--text-0`, `--text-1`, `--text-2` (text ramp)
- `--accent`, `--accent-2` (gradient endpoints)

A skin may define additional variables, but these 10 must
always be present so the theme system (0018) can re-tint any
skin at runtime. The `muzon-skin` validator parses `theme.css`
and asserts the 10 required variables are present.

The TZ §6 "skin quality is spaghetti" rule in code form:
**no hard-coded colours in component CSS, only via CSS
variables**. The 4 built-in skins honour this; the
`themes.html` design is the reference.

## What this decision locks for v0.2.0 / v0.3.0

- The `muzon-skin` crate (TZ §3.1.1) gets its first non-stub
  content: `SkinManifest`, `builtin_skins`, `default_skin`,
  `load_skin`, `validate`. The full loader for user-installed
  skins (ZIP / dir) lands in 0016.
- The 4 built-in skin directories live at
  `crates/muzon-skin/skins/<id>/`. Each has a `skin.toml`; the
  Modern Skin has the full HTML+CSS port from `themes.html`
  concept 1; the 3 others have `skin.toml` + `theme.css`
  placeholders that 0017 fills in.
- The default skin is `modern`. The `ui.default_skin` config
  field is the runtime switch; v0.1.0 already has the field
  in `muzon-core::UiConfig` and v0.2.0's Settings UI (0019)
  adds the picker.
- The mini-mode template per skin is a v0.3.0 deliverable
  (0020); the manifest declares `mini_template = ""` for now
  and the runtime falls back to cropping the main template.

## Non-goals for this decision

- Full HTML+CSS port of Winamp, Dense Pro, Cinematic skins:
  deferred to 0017 (v0.3.0).
- The mini-mode variant per skin: deferred to 0020 (v0.3.0).
- The theme system that re-tints the CSS variables at
  runtime: 0018 (v0.3.0).
- The Settings UI switcher that flips the default skin at
  runtime: 0019 (v0.3.0).
- User-installed skins (ZIP / dir) and the skin installer
  CLI: 0016 (v0.3.0).

## MCP replay block

The orchestrator run that produced this file tried to call
`memory.record_decision` and `memory.record_task_lineage_event`
for the 0010 child of the v0.2.0+ fan-out. The memory MCP
server returned `-32000 Internal error` on every call. When
MCP recovers, the next run must replay:

- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="Skins: 4 built-ins from themes.html; default = modern", dedup_key="MusicLab/decision/0005", details={tz_ref, source_issue_ref, source_design, default_skin, four_ids, grid_shapes, css_variable_contract})`
- `memory.record_task_lineage_event(task_ref="Issues/done/0010", stage="completion", agent_role="orchestrator", summary="0010 closed: skin system locked; modern skin ported; 3 others placeholder for 0017", commit_sha=<0010-commit-sha>)`
