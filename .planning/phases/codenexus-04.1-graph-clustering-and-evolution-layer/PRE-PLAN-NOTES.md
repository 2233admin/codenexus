# Phase 04.1 — Pre-Plan Locked Decisions

> **Purpose:** Decisions locked at add-phase time (2026-04-28) so `/gsd-plan-phase 04.1` doesn't re-litigate them. This file is **not** the PLAN.md (that comes from /gsd-plan-phase). It's upstream context: things /gsd-plan-phase should read first to skip dead-end paths.

> **Companion docs:** `~/.claude/plans/d-projects-codenexus-pasted-text-3-zazzy-newell.md` (full plan rationale); `.planning/STATE.md` Roadmap Evolution entry 2026-04-28; `.planning/PROJECT.md` line 67-68 (promoted-from markers).

---

## Phase 04.1 — Locked Scope Summary

**Title:** Graph Clustering and Evolution Layer
**Depends on:** Phase 4 (file-watcher / delta-diff harness; multi-language tree-sitter)
**Unblocks:** Phase 5 Bridge (cluster IDs as cross-domain entities); Phase 6 Reach (plugin slot for external clustering algos)

**Core deliverables (in priority order — Slice 1 first, gates Slice 2-3):**

1. Static Leiden module-boundary clustering — `petgraph` Rust binding
2. Static Infomap call-flow refinement — sub-clustering within Leiden communities
3. Incremental layer — empirical DF-Leiden vs ND-Leiden choice, 1.14×-1.37× speedup expected
4. CoDÆN-Polito eval harness — consume existing Python repo, not rebuild
5. A2A endpoint extension — `query_clusters` + `query_evolution` ops

**Explicitly OUT (deferred to Phase 6+):**
- HIT-Leiden (graph too small + no reference impl)
- GNN-hybrid (DLEC, neural Map Equation)
- Browser-side WASM Leiden
- Architecture-drift visualization UI (Phase 5 territory)

---

## Locked Algorithm Choices

### Static layer

| Algorithm | Crate / Source | Status | Notes |
|-----------|----------------|--------|-------|
| **Leiden** (module boundaries) | `petgraph` Rust | ✅ Adopt | Reuses `confidence: f64` on Calls edges as edge weight (ARCH §3.5.4) — zero added cost |
| **Infomap** (call-flow refinement) | `infomap` crate first; shell-out C++ fallback | 🔬 Spike | 30 min spike during plan-phase to evaluate crate quality |

### Incremental layer — algorithm choice is EMPIRICAL, not theoretical

Paper-grounded numbers (Sahu et al. 2024, "DF Leiden" arxiv extension):

| Algorithm | Speedup vs Static Leiden | Where it wins | Where it loses |
|-----------|--------------------------|---------------|----------------|
| **DF-Leiden** | **1.37×** on large random-batch updates | Synthetic / random batch updates | Real dynamic graphs (refinement+aggregation overhead) |
| **ND-Leiden** | **1.14×** on real dynamic graphs | Real commit-stream-like updates (CodeNexus's case) | Synthetic batch benchmarks |

**Implication:** "DF-Leiden is the default" in earlier plan drafts is **wrong**. Phase 04.1 Slice 1 must benchmark BOTH on actual CodeNexus commit-stream replay before committing to one. Real-world dynamic graphs (which CodeNexus is) may favor ND-Leiden despite DF having the higher headline number.

### HIT-Leiden — DEFER permanently for Phase 04.1

Three independent reasons:

1. **No reference implementation found.** Lin et al. 2026-Q1 paper, no public Python or Rust port at search-time. CodeNexus must not be the first port.
2. **Graph size argument.** HIT-Leiden's advertised 10²-10³× speedup is on 10⁶-node graphs where hierarchical-tree reuse pays off. CodeNexus poc.db has **2116 symbols** — below the regime. Even ideal HIT-Leiden would shave milliseconds off an already-fast static run.
3. **Cost-benefit.** Implementing HIT-Leiden from scratch = weeks; payoff on 2116-symbol graph ≈ imperceptible. Bad ROI.

Revisit only if: (i) reference impl appears on GitHub AND (ii) CodeNexus graph grows past ~10⁵ symbols (multi-repo registry phase, possibly Phase 5 Bridge).

---

## Locked Migration SQL

Copy verbatim into Slice 1's first commit:

```sql
-- experiments/poc-retrieval/migrations/0NN_add_community_id.sql
ALTER TABLE symbols ADD COLUMN community_id INTEGER;
-- initial value NULL; meaningful semantic = "not yet clustered"
-- Leiden run populates via:
--   UPDATE symbols SET community_id = ?2 WHERE id = ?1;
CREATE INDEX IF NOT EXISTS idx_symbols_community ON symbols(community_id);
```

**Why ALTER not drop-and-rebuild:**
- 2116 symbols × 8 min reindex (Phase 03.6 measured wall-clock) = wasted compute
- NULL is meaningful semantic for "Leiden hasn't run yet"
- Index supports the future `query_clusters` A2A op (covered query: `WHERE community_id = ?`)

**Why no NOT NULL constraint:**
- Newly indexed symbols start NULL until next Leiden run
- Forcing NOT NULL would couple symbol-insert to Leiden-run, breaking the file-watcher decoupling

---

## Locked Eval Harness

**Reference repo:** `SmartData-Polito/Dynamic_CommunityDetection_Benchmark` (Python)

- Implements LFR-evolving graph generator with **9 evolution events** (birth/death/merge/split/grow/shrink/continue/join/expand)
- Already has ground-truth labels — no need to derive them
- Setup cost: ~15 minutes (clone + `pip install -r requirements.txt` + sample run)

**Integration pattern:**
1. Phase 04.1 calls Python repo as CLI subprocess from Rust integration test
2. Output is JSON / edgelist files
3. Rust side parses + feeds into Leiden / DF-Leiden / ND-Leiden
4. NMI computed via `cluster_metrics` Rust crate or hand-rolled (NMI is ~30 LOC)

**Do NOT:**
- Rebuild LFR-evolving generator from scratch
- Adopt NeGMA as a production algorithm — we use Leiden, NeGMA is just CoDÆN's competitor in their paper, irrelevant to us

**REQ-10 eval infra (B1-B7 baseline subset)** is **complementary**, not replaced:
- REQ-10 / B1-B7 = production-side precision@5 hold-out (correctness of retrieval)
- CoDÆN-Polito = community-detection correctness (NMI vs ground-truth)
- Both feed Phase 04.1's success gates (G1-G5); neither subsumes the other

---

## Success Gates (calibrated)

| Gate | Metric | Threshold | Source | Calibration note |
|------|--------|-----------|--------|------------------|
| **G1** Static quality | Modularity (Leiden) on poc.db (2116 symbols) | ≥ 0.6 | self-test | Standard Leiden modularity floor |
| **G2** Static quality | NMI vs ground-truth on LFR-1K | ≥ 0.85 | CoDÆN-Polito | Standard incremental-clustering bench threshold |
| **G3** Incremental correctness | Chosen algo (DF or ND) labels match full rerun on 100-commit replay | NMI ≥ 0.95 | self-test | Tight — incremental should be near-identical to full rerun |
| **G4** Incremental speed | Chosen algo update on +1 commit (5-50 edge delta) | < 200ms p95 | self-test | **Marginal** at 1.37×; if static <150ms, drop incremental entirely |
| **G5** Eval reproducibility | CoDÆN-Polito Python bench scripted + Rust consumer reproducible on CI | scripted | repo + thin Rust wrapper | 15-min setup, not new harness |

---

## Slice Breakdown Hint (for /gsd-plan-phase 04.1)

A reasonable first-pass slicing — actual /gsd-plan-phase output may differ:

| Slice | Scope | Gates |
|-------|-------|-------|
| **04.1.1** Static Leiden + migration | `clustering.rs` + ALTER TABLE + Leiden-on-poc.db smoke test | G1, baseline timing for G4 calibration |
| **04.1.2** CoDÆN harness wiring | Python subprocess + Rust JSON consumer + NMI computation | G2, G5 |
| **04.1.3** Incremental algo spike + decision | Implement DF + ND in parallel, benchmark on commit-stream, pick winner | G3, G4, OR decision: drop incremental |
| **04.1.4** Infomap refinement | Crate spike, fallback shell-out, integrate with Leiden output | (no new gate, quality bonus) |
| **04.1.5** A2A ops + closure | `query_clusters` + `query_evolution` schema + Go side + closure SUMMARY | All gates re-verified on closure |

Gate sequencing: **04.1.1 must reveal static baseline timing before committing to 04.1.3.** If static <150ms, 04.1.3 is dropped; phase ships at 4 slices.

---

## What This Document Is Not

- **Not** PLAN.md — that's the output of `/gsd-plan-phase 04.1`
- **Not** RESEARCH.md — that's the output of `/gsd-research-phase 04.1` (skip if not needed; the linked sources here are sufficient research)
- **Not** binding on /gsd-plan-phase to slice exactly this way — it's a hint, not a contract. If plan-phase finds a better breakdown, override.

What it **is**: locked answers to "do we DF-Leiden or ND-Leiden" / "do we HIT-Leiden" / "what does the migration SQL look like" — questions that already burned conversation cycles. Don't burn them again.
