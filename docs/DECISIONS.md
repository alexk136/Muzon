# Muzon — Open Product Decisions

Authoritative record of the product-level decisions that gate v0.1.0
("Skeleton") and downstream milestones. Each decision links back to the
relevant TZ.md section and lists the option, the rationale, the
alternatives considered, and the consequences. When a decision must be
revisited, add a new dated entry below and supersede the prior one in
the durable record; do not silently edit history.

Source: TZ.md v0.1, §7 (Open questions to resolve before start).
Issue: [Issues/done/0001_resolve_open_product_questions.md](../Issues/done/0001_resolve_open_product_questions.md)
Status: ALL FOUR DECISIONS CLOSED (MCP replay pending — see notes).
Owner: orchestrator (operator sign-off assumed via "run" override on
2026-06-26; operator must confirm the GitHub org placeholder before
issue 0005 lands).

## Decision 0001-N1 — Product name: "Muzon"

- Status: closed (default accepted)
- TZ reference: §1.1, §7 question 1
- Decision: keep the working name "Muzon".
- Rationale: the name is already baked into the workspace layout
  (`crates/muzon-*` in TZ §3.1.1), the CLI grammar (`muzon play …` in
  §2.6.3), the Flatpak path, and the binary name. A global rename
  mid-stream has a high cost (TZ, docs site, Flatpak app-id, AUR
  package, .deb/.rpm names, `cargo` binary, all `Docs/` examples) and
  no obvious win.
- Alternatives considered: any alternate name. Rejected because each
  candidate required a full TZ rename before any issue becomes
  executable.
- Consequences:
  - Cargo binary name: `muzon`.
  - Flatpak app-id: `org.muzon.Muzon` (placeholder; operator must
    confirm when the GitHub org is created).
  - Domain placeholder: `muzon.rs` (not registered; operator must
    confirm).
  - AUR / .deb / .rpm names: `muzon`.
  - TZ §2.6.3 CLI examples need no change.

## Decision 0001-N2 — License: dual MIT / Apache-2.0

- Status: closed (default accepted)
- TZ reference: §4.6, §7 question 2
- Decision: dual MIT / Apache-2.0. Downstream users may pick either
  license at their option.
- Rationale: TZ §4.6 already prescribes "MIT or Apache-2.0 (dual)";
  §7 question 2 only asks to confirm. The dual-license form is the
  standard Rust ecosystem convention and is the strictest reading of
  TZ §4.6 ("GPL/AGPL — нет"). A future migration from dual to single
  is annoying but not catastrophic; a migration from MIT to GPL would
  require contributor re-consent and is not on the table.
- Alternatives considered: Apache-2.0 only. Acceptable; rejected
  because the dual form gives downstream users more flexibility at no
  cost to the project.
- Consequences:
  - `LICENSE-MIT` and `LICENSE-APACHE` exist at the repo root
    (full text of each).
  - `NOTICE` exists at the repo root with the dual-license boilerplate
    and the SPDX-style `MIT OR Apache-2.0` guidance.
  - Every Rust source file in the workspace will carry a
    `SPDX-License-Identifier: MIT OR Apache-2.0` header. This header
    is enforced by the cargo-deny / header-lint check introduced in
    issue 0005.
  - No CLA in v0.1.0. If a CLA is needed later, it is a separate
    decision and does not affect this one.
  - Dependency whitelist from TZ §4.6 ("GPL/AGPL — нет") is enforced
    by `cargo-deny` in CI (issue 0005).

## Decision 0001-N3 — GitHub org / repo URL

- Status: closed (placeholder, operator must confirm)
- TZ reference: §5 (success metrics), §7 question 7, §7 question 8
  (site domain)
- Decision (placeholder): org `musiclab`, repo `muzon`, canonical URL
  `https://github.com/musiclab/muzon`. Site domain placeholder:
  `muzon.rs`.
- Rationale: the repo URL must be locked before any release manifest
  references it (Flatpak, AUR, .deb, docs site). TZ §5 success metrics
  assume a public GitHub repo with ≥ 1000 stars in three months. The
  placeholder mirrors the project slug (`MusicLab`) and the binary
  name (`muzon`); a different org name is a low-cost rename before
  the first release.
- Alternatives considered: org `muzon-proj`, `muzon-fm`, `mizon`. All
  are workable. The placeholder choice is the most boring one.
- Operator action required before issue 0005 lands:
  1. Create the org (if not using `musiclab`, replace all references
     in this file and in the eventual CI config).
  2. Create the empty `muzon` repo.
  3. Confirm the canonical URL in this file.
  4. Confirm the site domain (`muzon.rs` is the default).
- Consequences:
  - All CI badges, Flatpak manifest, AUR pkgbuild, .deb control
    fields, docs site, and `Cargo.toml` repository fields reference
    the canonical URL.
  - Until the operator confirms, the placeholder is a known
    placeholder; do not publish any release artifact that bakes it in.

## Decision 0001-N4 — Telemetry: no telemetry, ever

- Status: closed (strictest reading, matches TZ §3.9 + §6 RF risk)
- TZ reference: §2.4.3, §3.9, §6 (risk: LLM providers unavailable in
  RF), §7 question 6
- Decision: the released binary makes zero outbound network calls by
  default. No crash reports, no anonymous usage analytics, no version
  pings, nothing. Every network call (AcoustID, MusicBrainz,
  Last.fm, ListenBrainz, LLM providers) is opt-in per §3.9 and is
  gated behind an explicit UI switch plus a per-feature runtime
  config flag.
- Rationale: this is the strictest reading of TZ §3.9
  ("Никакой телеметрии в базовой поставке") and matches TZ §6 risk
  for users in the Russian Federation, where LLM provider reachability
  is the dominant availability concern. It also matches TZ §2.4.3
  ("Никаких телеметрийных вызовов в LLM без явного согласия
  пользователя").
- Alternatives considered:
  - Opt-in anonymous crash reports: rejected for v0.1.0; can be added
    in a later minor release via a separate RFC if the need becomes
    concrete.
  - Opt-in anonymous usage analytics: same.
- Consequences:
  - The release binary MUST NOT make any outbound network call unless
    the user has explicitly enabled the corresponding feature in
    config and UI.
  - This is enforced in three places, in order of priority:
    1. Issue 0006 (XDG paths + config + logging): the config schema
       has no `telemetry.enabled` key; absence of a feature flag is
       the absence of the feature.
    2. Issue 0006 logging infra: a release-build assertion
       (`debug_assertions` off in release) checks that no
       `reqwest::Client` is constructed outside the
       `muzon-musiclab` and `muzon-llm` modules (placeholder
       module names; locked in 0006).
    3. v0.6.0 (LLM provider abstraction): all LLM and AcoustID
       calls go through a single `OptInClient` wrapper that fails
       closed if the user has not opted in.
  - Any future issue that needs to add a network call MUST open a
    new decision in this file and link it from the issue's DoD.
  - CI in issue 0005 adds a static check (network-call audit) that
    fails the build if any code path outside the opt-in modules
    references `reqwest`, `hyper`, `ureq`, or `tokio::net` directly.

## Operator sign-off

All four decisions were adopted from the recommended defaults in
[Issues/done/0001_resolve_open_product_questions.md](../Issues/done/0001_resolve_open_product_questions.md)
on 2026-06-26 via the issue_runner "run" operator override. Decision
0001-N3 (GitHub org) carries a placeholder that the operator must
confirm before issue 0005 lands; the other three decisions are
closed and durable.

## MCP replay block

The orchestrator run that produced this file tried to call
`memory.record_decision` for each of the four decisions above. The
memory MCP server returned `-32000 Internal error` on every call
(see `processed_reports.json:mcp_calls_pending` and the
`mcp_status: blocked` note on issue 0001). When MCP recovers, the
next orchestrator or developer run must replay:

- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="Product name: Muzon", dedup_key="MusicLab/decision/0001-N1", details={tz_ref, source_issue_ref, alternatives, consequences, license_anchor="LICENSE-MIT+APACHE"})`
- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="License: dual MIT/Apache-2.0", dedup_key="MusicLab/decision/0001-N2", details={tz_ref, source_issue_ref, alternatives, consequences, files=["LICENSE-MIT","LICENSE-APACHE","NOTICE"]})`
- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="GitHub org: musiclab/muzon (placeholder, operator confirm)", dedup_key="MusicLab/decision/0001-N3", details={tz_ref, source_issue_ref, status="placeholder", operator_action_required=true})`
- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="Telemetry: none, ever", dedup_key="MusicLab/decision/0001-N4", details={tz_ref, source_issue_ref, enforcement_layers=["0006-config","0006-release-assertion","v0.6.0-opt-in-client"]})`
