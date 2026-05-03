---
phase: 5
artifact: discuss-strategic (G1 + G7 only)
status: PROPOSED (locks pending Curry sign-off)
authority: BETA-V1-SPEC v1.0 (frozen 2026-05-02) + drift probe SUMMARY (2026-05-03)
covers: G1 (memU integration mode), G7 (V1.0 vs V1.1+ cut line)
defers: G2-G6 (separate strategic round; not blockers for G1+G7 lock)
authored: 2026-05-03
authored_by: gsd-discuss-advisor (Claude Opus 4.7)
---

# Phase 5 Discuss -- Strategic (G1 + G7)

This artifact resolves two coupled gray areas. G2-G6 (op signatures, schema
shape, ADR scope, MCP affordance copy) are tactical and resolved in a
follow-up discuss round; they do not gate the G1+G7 lock.

Evidence anchors used throughout:
- BETA-V1-SPEC.md sec 6 line 163 (V1.1+ backlog: shared-PG memU coupling)
- BETA-V1-SPEC.md sec 8 lines 211-223 (Phase 5 scope, OUT-of-scope list)
- PROJECT.md line 145 (current memU integration self-contained store pin)
- PROJECT.md line 107 (Strategic bet 2 cross-session annotations)
- PROJECT.md line 108 (Strategic bet 3 ADR semantic indexing via query_constraints)
- 2026-05-03 drift probe SUMMARY (M5_fnk = 1.0 on path,name,kind keying)
- D:/projects/memU/pyproject.toml (memu-server entry-point declared)
- D:/projects/memU/src/memu/ (no server/ module; library-first today)
- D:/projects/memU/docs/sqlite.md (SQLite backend, brute-force vector, ~100k cap)
- D:/projects/memU/docs/architecture.md lines 32-50 (MemoryService composition root)


---

## G1: memU integration mode for Phase 5 V1.0

### Restate

CodeNexus PROJECT.md line 145 currently pins memU integration as
self-contained store, with Phase 5 (Bridge) potentially revisiting fused
recall via shared PG. BETA-V1-SPEC sec 6 line 163 explicitly OUTs
shared-PG memU coupling from V1.0. But memU integration is broader than
shared PG -- candidates include in-process Python embedding, FFI, HTTP
A2A, shared SQLite mount, or zero-coupling-with-export-hook. We must
lock which mode V1.0 actually ships and which deferred mode V1.1+
migrates to.

### Current state of memU API surface (verified 2026-05-03)

memU is library-first Python, not a service today:

- Composition root is memu.app.service.MemoryService -- a Python class
  constructed in-process with llm_profiles + database_config dicts
  (memU/docs/architecture.md lines 32-50; src/memu/app/service.py lines 49-95).
- pyproject.toml declares scripts entry memu-server = memu.server.cli:main
  BUT src/memu/server/ directory does not exist in HEAD. The HTTP server
  entry point is declared-but-unimplemented. No FastAPI / uvicorn / HTTP
  surface ships today.
- Backends: sqlite (default, brute-force cosine, ~100k item cap per
  docs/sqlite.md line 89) and postgres (pgvector, optional extra).
- No FFI (Python only). No A2A endpoint. No native Rust binding.
- Surface relevant to CodeNexus: memorize(...) ingests Resources;
  retrieve(...) returns ranked items; CRUDMixin gives manual create/update/delete.
  None of these is "remember this annotation against this code symbol id".
  CodeNexus would have to encode (file, name, kind) into Resource/MemoryItem
  metadata and reverse-decode at retrieval.

Implication: any in-process integration today requires CodeNexus's Rust/Go
binary to embed CPython OR shell out to a Python subprocess. Neither is
compatible with the single fat-binary, zero install dependencies
distribution constraint (PROJECT.md line 123-124). HTTP coupling is also
not available today (server module missing).


### Options table

| Mode | V1.0-fit | V1.0 cost | V1.1+ migration cost | Risk |
|------|----------|-----------|----------------------|------|
| A. Self-contained pure (no memU touch) | HIGH | low (own SQLite for notes; reuse Phase 2 spike storage decision) | medium (must add export hook + key-mapping at V1.1+ entry) | low (no external dep; zero coupling means zero coupling-risk) |
| B. Self-contained + V1.1-ready export hook | HIGH | low+ (own SQLite + emit JSONL memU-shaped event log on every write; never read it back in V1.0) | LOW (V1.1 reader replays JSONL into memU) | low (export hook is write-only, no coupling) |
| C. In-process Python embed (PyO3 / subprocess shell-out) | LOW | high (PyO3 binding OR Python subprocess management; breaks fat-binary distribution) | n/a (this IS the integration) | high (Python runtime in distribution; CPython version coupling; perf unknown) |
| D. HTTP A2A coupling to memu-server | NONE | impossible-today (memu-server module does not exist in HEAD) | medium (when memU ships server, CodeNexus adds HTTP client) | high (depends on upstream memU work that hasn't happened) |
| E. Shared PG (CodeNexus + memU both write same Postgres) | NONE (explicit V1.1+ in BETA-V1-SPEC sec 6) | very high (forces PG dependency, breaks fat-binary, breaks stranger on clean machine MUST 2) | n/a (this IS the V1.1+ destination per current pin) | very high (operational complexity; schema co-evolution) |

### Recommendation: Mode B -- self-contained + V1.1-ready export hook

Rationale: Mode A (pure self-contained) satisfies V1.0 with the lowest
implementation cost but pays a non-trivial migration tax at V1.1+ entry
(must add the export hook AND backfill the existing notes). Mode B pays
that tax up front (~half-day of work) by emitting a write-only JSONL event
log on every remember_symbol_note write, shaped so that a future V1.1+
reader can replay events into memU's MemoryService.memorize(...) API
without re-reading CodeNexus's storage. This buys two strategic options
cheaply: (1) memU integration becomes a V1.1+ pure-additive feature with
no V1.0 schema migration; (2) if V1.1+ adopts a different memory backend
(memorix / Mem0 / custom), the same JSONL is replayable elsewhere. Modes
C, D, E are all incompatible with V1.0 constraints (single-binary distribution,
stranger on clean machine MUST 2, BETA-V1-SPEC sec 6 explicit OUT).
The cost delta A->B is small enough (~4hr work, ~0 ongoing maintenance) that
deferring it to V1.1+ would be false economy -- the JSONL writer is
trivial; the schema design is the work, and the schema design has to
happen at Phase 5 PLAN.md time anyway since remember_symbol_note
schema (G3) is being locked there.


---

## G7: V1.0 vs V1.1+ cut line for Phase 5 sub-features

### Restate

BETA-V1-SPEC sec 8 lines 222-223 explicitly OUTs Obsidian wiki / shared PG
/ IDE affordances / remote A2A / clustering. PRE-PLAN-NOTES UQ2 asks: is
memU integration itself V1.0 or V1.1+? PRE-PLAN-NOTES UQ4 asks: must
Phase 5 wait for edges to populate (currently 0 across both corpora per
drift probe SUMMARY) or can it ship symbol-only? UQ5 asks: is ADR
extraction the same as query_constraints under two names?

### Decision table (Phase 5 sub-feature x cut line)

| Sub-feature | V1.0 | V1.1+ | V1.2+ | Rationale |
|-------------|------|-------|-------|-----------|
| remember_symbol_note write/read/list | YES | -- | -- | Core MUST 5; memU NOT required (per G1 Mode B). Self-contained SQLite owns. |
| remember_symbol_note supersede + audit log | YES | -- | -- | Cheap to ship together; defers nothing useful. Append-only history fits Mode B JSONL. |
| query_constraints(file) + query_constraints(symbol) | YES | -- | -- | MUST 5 explicit. Symbol-keyed by (path, name, kind), drift probe M5_fnk = 1.0. |
| query_constraints(topic) semantic NL | YES (BM25 reuse) | semantic vector | -- | Reuse search.rs::search BM25 path; vector NL search for topic defers (extra eval surface). |
| get_edit_context(symbol) composite | YES | -- | -- | MUST 5 explicit. Compose existing list_callers + query_constraints + list_notes. |
| get_edit_context(file) aggregation | YES | -- | -- | Same composite, file-scoped. |
| ADR extraction (markdown MUST/MUST-NOT/SHOULD) | YES | -- | -- | PROJECT.md line 108 strategic bet 3. Stored as constraint rows, surfaced through query_constraints. |
| ADR extraction trigger: on-demand CLI | YES | -- | -- | Manual trigger only in V1.0. |
| ADR extraction trigger: file-watch / scheduled | -- | YES | -- | Pure UX polish; manual re-extract is acceptable for MVP. |
| MCP tool wrapping (3 ops) | YES | -- | -- | MUST 5 implicit (the public surface is MCP). |
| MCP tool affordance polish (CodeCompass <=5% skip target) | partial (descriptions only) | YES (eval-driven iteration) | -- | V1.0 gets first-cut descriptions; eval-driven tuning needs the 30-task harness which is MUST 6 work, not Phase 5 work. |
| memU integration: in-process / HTTP / shared PG | -- | -- | -- | Per G1: Mode B export hook ships V1.0; actual memU coupling is V1.1+ at the earliest, V1.2+ if memu-server module hasn't shipped upstream. |
| memU export hook (write-only JSONL) | YES | -- | -- | Per G1 recommendation. Cheap insurance. |
| Edge-aware ops (list_callers integration into get_edit_context) | YES (degrades gracefully if edges=0) | -- | -- | Resolves UQ4: ship symbol-only NOW; when 04.5-03 W3 lands edges, get_edit_context auto-enriches. No code change at Phase 5 boundary. |
| Obsidian wiki graph integration | -- | -- | YES | OUT per BETA-V1-SPEC sec 6. Lives in obsidian-llm-wiki side anyway. |
| IDE affordances beyond MCP | -- | -- | YES | OUT per BETA-V1-SPEC sec 6. |
| Remote A2A mesh | -- | -- | YES | OUT per BETA-V1-SPEC sec 6. |
| Clustering / evolution layer | -- | -- | YES | OUT per BETA-V1-SPEC sec 6 (Phase 04.1 scope, deferred). |
| Cross-binary-version drift probe | YES (deferred test, not feature) | -- | -- | UQ3: M5_fnk = 1.0 evidence is enough to commit (path, name, kind) NOW; cross-version probe is a follow-up validation, not a Phase 5 feature gate. Run it after W3 ships. |


### Recommendation: ship V1.0 wide on op surface, narrow on integration; defer all coupling

V1.0 = full Phase 5 op surface (3 A2A ops + ADR extraction + MCP wrap +
write-only memU export hook), self-contained storage, NO live memU
coupling. V1.1+ = first-class memU integration replay (when upstream
memu-server module ships OR PyO3 path opens). V1.2+ = everything
explicitly OUT-d in BETA-V1-SPEC sec 6 (Obsidian, IDE, remote A2A,
clustering). Rationale: MUST 5 in BETA-V1-SPEC requires the three named
ops to ship as the Beta V1 memory-assisted edit surface; cutting any of
them collapses the audit's load-bearing reframe (PROJECT.md Strategic).
The op surface IS the value proposition for V1.0; the storage backend
swap is a V1.1+ implementation detail. UQ4 resolves to ship now, edges
arrive transparently because get_edit_context's edge component degrades
gracefully (returns empty caller list when edges=0), so Phase 5 does not
have to wait on 04.5-03 W3. UQ5 resolves to ADR extraction populates
the constraint store; query_constraints reads from it -- they are
producer/consumer of the same data, not the same op. This means ADR
extraction is upstream of query_constraints in the data flow and ships
in the same V1.0 wave but is a distinct sub-feature.

---

## Cross-coupling (G1 vs G7)

G1 Mode B (self-contained + JSONL export hook) and G7 (ship op surface
wide, defer coupling) are mutually reinforcing -- the export hook is
ONLY useful if the V1.1+ coupling cut line (G7) is deferred but
planned, not deferred indefinitely. If G7 had said memU integration
is V1.0, G1 would have to choose Mode C/D and break the fat-binary
constraint. If G7 had said memU integration is V2.0+ / never, G1's
export hook would be dead weight (Mode A would be optimal).

The chosen pairing makes the V1.0 -> V1.1+ migration boundary cheap:
- G1 Mode B writes notes to local SQLite AND emits memU-shaped JSONL events.
- G7 V1.1+ reads JSONL events and replays into memU's memorize(...) API
  when memu-server module ships upstream OR when CodeNexus accepts
  Python-runtime coupling.
- Migration = pure additive: V1.0 binary keeps working post-V1.1+ launch
  for users who don't want the coupling; V1.1+ users opt in via config flag.

The reverse coupling matters too: if Phase 5 V1.0 ships and the JSONL
schema is wrong (G3 schema decision), the V1.1+ migration stays cheap
because the JSONL is write-only -- a re-export tool can read SQLite
notes back and re-emit corrected JSONL. SQLite is the source of truth;
JSONL is the bridge contract. This decouples G3 schema risk from G1+G7
lock risk.


---

## Open questions for Curry

These are the questions only Curry can decide; they are NOT blockers for
G1+G7 lock but should be resolved before Phase 5 PLAN.md commits the
W0-W6 wave breakdown.

1. JSONL export hook destination: write to a CodeNexus-owned dir
   (~/.codenexus/memu-export/notes.jsonl) or to a user-configurable
   path with sensible default? V1.1+ migration is easier with the
   former; user observability is easier with the latter. Recommend the
   former + config override; needs Curry yes/no.

2. memU upstream relationship: do you want CodeNexus to file an
   issue with memU upstream requesting (a) the missing memu/server/
   module ship for V1.1+ coupling, OR (b) a stable JSONL ingestion
   contract that CodeNexus can target? Either reduces V1.1+ migration
   risk; both is overkill.

3. PG dependency tolerance for V1.1+: BETA-V1-SPEC sec 6 OUTs
   shared-PG from V1.0. Is V1.1+ shared PG opt-in for power users
   acceptable, or is it permanently OUT (in which case the V1.1+ memU
   coupling target is sqlite-backed memU only)? Recommend: V1.1+ allows
   sqlite-backed memU coupling only; shared-PG stays V1.2+ behind a
   separate ADR.

4. Drift probe cross-version follow-up timing: probe SUMMARY P1
   gap C3 says cross-binary-version drift NOT tested. Block Phase 5
   V1.0 ship on running this probe (delays Beta), or treat as a P2
   follow-up that ships post-V1.0 alongside the eval harness? Recommend:
   P2 follow-up, since M5_fnk = 1.0 on same-binary re-runs is sufficient
   evidence for storage key lock per spec decision rule, and cross-version
   drift first becomes meaningful when there IS a meaningful pre/post
   pair (i.e. after W1+ ships an indexer change).

5. Mode B JSONL schema authority: should the JSONL event schema be
   designed to match memU's Resource/MemoryItem shape exactly (per
   memU/docs/architecture.md lines 13-18) so V1.1+ replay is trivial,
   OR designed for CodeNexus's domain (Symbol-keyed) and adapted at
   replay time? Recommend: match memU shape (Resource = the symbol
   itself; MemoryItem = the note text; metadata carries (path, name, kind)
   for round-trip identity). This is a Phase 5 PLAN.md G3 decision but
   G1 Mode B presumes the answer is match memU shape, so flagging.

---

## Honest gap list (rule 18)

- P1: G2-G6 not resolved here. Phase 5 PLAN.md is blocked on a
  separate strategic round for op signatures (G2), schema shape (G3),
  composite output format (G4), ADR scope (G5), MCP affordance copy
  (G6). G1+G7 lock is a precondition for those discussions but not
  sufficient on its own.
- P2: Mode B export hook implementation cost estimate (~4hr) is
  one-author guess. Plan-checker iter 2 should validate against Phase 2
  spike storage decision (redb vs rusqlite+sqlite-vec) since JSONL
  emission path differs slightly between the two.
- P2: memU upstream server/ module status not verified beyond
  directory missing in HEAD as of 2026-05-03. A WebSearch + GitHub
  issue scan would tighten the V1.1+ coupling timeline estimate.
- P3: Open questions 1-5 above are Curry-decidable; deferring them
  past PLAN.md authoring risks PLAN drift. Recommend resolving all 5
  before W0 starts.
