# Decision 0004 — Audio embedding model: tiered (CLAP primary, Panns secondary)

Status: closed (recommended default adopted).
TZ reference: §2.4.2, §3.5.2, §6, §7 question 4.
Source issue: [Issues/done/0004_finalize_audio_embedding_model.md](../../Issues/done/0004_finalize_audio_embedding_model.md).
Date: 2026-06-26.
Owner: orchestrator (developer spike owner per issue 0004 routing).
Blocking: gates v0.5.0 "MusicLab Core" only; v0.1.0–v0.4.0 are not affected.

## Decision

Adopt a **tiered embedding strategy** for v0.5.0 "MusicLab Core":

- **Primary**: LAION-CLAP (HTSAT-base backbone), Apache-2.0. Used
  for both text-aligned queries ("что-то для поздней ночи") and
  similarity search. Same `ort` ONNX runtime as the secondary.
- **Secondary**: Panns_inference, MIT, ~50 MB. Used for
  instrument/mood tags as a fallback when the CLAP inference is
  too slow on the user's hardware.
- **Removed from shortlist**: MusiCNN / Musicnn (redundant with
  Panns for our purposes), vanilla CLAP without LAION
  re-distillation (less aligned with text), MS-CLAP 2022 checkpoint
  (superseded by LAION-CLAP).

The two models are complementary, not alternatives: CLAP for text
and similarity, Panns for instrument/mood tags. The ORT runtime,
model loader, and storage layer (`sqlite-vec`) are shared; only
the inference call differs.

## Why tiered, not CLAP-only

TZ §6 explicitly names "CLAP is too heavy" as a real risk. The
mitigation in §6 is "fallback на фичи-based поиск". A tiered
strategy operationalises that fallback as a real user-facing mode:
the user picks "tag-only" in config and the system runs Panns
instead of CLAP. The ORT runtime, model loader, and vector store
are the same in both modes; only the inference call changes. The
tiered path costs < 100 MB of model weight and a thin
`EmbeddingModel` enum.

## What this decision locks for v0.5.0

- The `muzon-musiclab` crate (TZ §3.1.1) loads both models via
  `ort` against ONNX exports.
- The persistence layer uses `sqlite-vec` for vector storage.
- The query surface is "find similar to track X" (CLAP cosine) and
  "find by text query" (CLAP text-aligned).
- The tag surface is "find tracks tagged <tag>" (Panns classifier
  output) and is always available regardless of tier.
- The tiered path is a user choice, not an automatic fallback:
  the user picks "tag-only" in config to opt into the lighter
  path on hardware that cannot run CLAP at the §3.7 budget.

## Why CLAP is the primary

TZ §3.5.2 recommends CLAP and the §2.4.2 text query ("что-то для
поздней ночи") only works on language-aligned embeddings. Panns and
MusiCNN are not language-aligned; a Panns-only path would need a
separate text-to-tag translator that does not exist yet and would
just shift the cost into the LLM layer (v0.6.0). CLAP gives both
similarity and text search out of the box.

## License compatibility (TZ §4.6)

- LAION-CLAP HTSAT-base: Apache-2.0. ✓
- Panns_inference: MIT. ✓
- All other §3.5.2 candidates: also MIT/Apache/BSD, also eligible.

## Perf budget (TZ §3.7)

The §3.7 "fingerprinting ≥ 50x realtime" line applies to the audio
fingerprint (AcoustID hash, v0.4.0), not to the embedding
extraction. The two are different work products. The real budget
for embeddings is:

- Throughput: embed a 4-minute track in < 30 s on CPU.
- RAM: < 500 MB with the MusicLab map loaded (full collection).
- Total: < 1.5 GB peak RSS during a 1000-track embedding run.

The v0.5.0 spike (deferred) confirms these budgets with a 200-line
Rust POC that embeds 50 tracks of varied length and genre and
records per-track latency, peak RAM, and total wall time.

## Spike scope (deferred to v0.5.0)

The 1-2 week spike from issue 0004 is explicitly **out of scope
for v0.1.0–v0.4.0**: the embedding work is a v0.5.0 milestone item
per TZ §5. The spike gates v0.5.0 and must produce:

1. **Model + runtime path confirmation.** Both LAION-CLAP and Panns
   load via `ort` and produce a deterministic embedding for a fixed
   PCM input. The output is stable across Intel, AMD, and (if
   relevant) Apple Silicon CPUs.
2. **Throughput + memory POC.** A 200-line Rust program that loads
   the chosen model(s) via `ort`, embeds 50 tracks of varied length
   and genre, and records per-track latency, peak RAM, and total
   wall time. Asserts the per-track latency fits the "embed a
   4-minute track in < 30 s on CPU" target on a reference machine.
3. **Persistence + query POC.** Embeds a small collection (50
   tracks), stores the vectors in `sqlite-vec`, runs a "find
   similar to track X" query and a text query ("chill piano"), and
   asserts the top-5 results are sane. This locks the §2.4.2
   contract.

If the spike shows that CLAP is too slow on the reference machine
even on GPU, the spike recommends whether to ship Panns-only as
the v0.5.0 default (text search deferred to a later minor release)
or to ship tiered. The orchestrator arbitrates.

## Non-goals for this decision

- The vector index library (`usearch` vs `sqlite-vec`): v0.5.0
  implementation issue, not part of this spike. The default in
  this decision is `sqlite-vec` because TZ §3.3 already names it.
- The dimensionality reduction algorithm (UMAP vs t-SNE): v0.5.0
  implementation issue, not part of this spike.
- Model registry / model download UX: v0.5.0 implementation issue.

## MCP replay block

The orchestrator run that produced this file tried to call
`memory.record_decision` and `memory.record_task_lineage_event` for
the 0004 child of the umbrella. The memory MCP server returned
`-32000 Internal error` on every call. When MCP recovers, the next
run must replay:

- `memory.record_decision(project_slug="MusicLab", agent_role="orchestrator", summary="Embedding model: tiered (CLAP primary, Panns secondary)", dedup_key="MusicLab/decision/0004", details={tz_ref, source_issue_ref, primary="LAION-CLAP/HTSAT-base", secondary="Panns_inference", runtime="ort", store="sqlite-vec", gates="v0.5.0"})`
- `memory.record_task_lineage_event(task_ref="Issues/done/0004", stage="completion", agent_role="orchestrator", summary="0004 closed: tiered embedding (CLAP+Panns); spike deferred to v0.5.0", commit_sha=<0004-commit-sha>)`
