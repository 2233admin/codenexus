# Phase 4 First Slice -- SPEC Interview Log (Checkpoint)

**Status: RESOLVED 2026-04-28.** User answered Q5=B + Q6=B same session after Jina key persistence + jina-cli verification detour. Final ambiguity 0.156 (gate ≤0.20 PASS, all 4 dimensions above minimum). `04-SPEC.md` + `04-PRE-PLAN-NOTES.md` written and committed alongside this log.

**Final delivery:**
- `04-SPEC.md` -- 5 R locked + boundaries + co-location rule + 13 acceptance checkboxes (incl. E2E smoke + eval no-regression) + 3 ASSUMPTION CORRECTIONS + interview log table
- `04-PRE-PLAN-NOTES.md` -- 5 hints (revision-fetch path, progress indicator path, R4/R5 mechanical patch sketch, E2E harness pieces, eval no-regression check) + planner planning surface (8 files touched)
- This `INTERVIEW_LOG.md` -- preserved as session log; no longer a checkpoint

**Next steps for user:** `/gsd-discuss-phase 4` -> implementation decisions (HOW); then `/gsd-plan-phase 4` -> 2-3 executable plans grouped per PRE-PLAN-NOTES Hint 5.

---

## Original checkpoint state (preserved for audit / replay)

---

## Phase 4 SPEC Range (Round 1 locked)

**Scope:** Phase 4 first slice = **first-run UX cluster + P2 resilience same-crate**

**Co-location rule (boundary protector):** P2 resilience entries (`server.rs:198`, `search.rs:31`) enter SPEC ONLY because they live in the same crate (`experiments/poc-retrieval/src/`) as `embedder.rs`. If a P2 touchpoint is in another crate (e.g. `core/` placeholder, Go `server/`), it does NOT belong in this SPEC. This is the geometric anti-scope-creep rule.

**Out of this SPEC (deferred to later slices/phases):**
- multi-language tree-sitter (Phase 4 group 2, separate SPEC)
- multi-repo registry (Phase 4 group 3)
- git overlay via gix (Phase 4 group 4)
- pattern detection / security scanners ported from CodeFlow MIT (Phase 4 group 5)
- tracing framework migration (PROJECT.md line 75 假设错误, see ASSUMPTION CORRECTION #1; tracing migration = separate quick task)
- EmbedError enum design (taxonomy 设计推到 future production-grade phase, this slice 不引入)

---

## ASSUMPTION CORRECTIONS (Round 1 locked, must appear in 04-SPEC.md)

1. **tracing**: PROJECT.md line 75 assumes tracing is already wired.
   VERIFIED FALSE: 0 uses of tracing, 33 `eprintln!` in `experiments/poc-retrieval/src/`.
   SPEC IMPACT: first-run UX improvements use `eprintln!`, not `tracing::info!`.
   Migration = deferred, explicitly out of scope.

2. **revision API**: PROJECT.md line 73 assumes `Qwen3TextInitOptions` exposes a `revision` parameter.
   VERIFIED FALSE: actual signature is `Qwen3TextEmbedding::from_hf(MODEL_REPO, &Device::Cpu, DType::F32, MAX_LEN)` -- 4 args, no revision (verified via `experiments/poc-retrieval/src/embedder.rs:72` and Phase 03.6 03.6-01-SUMMARY.md line 153-188 cargo cache spelunking).
   SPEC IMPACT: lock WHAT only ("revision MUST be pinned"). HOW = planner's job. Planner hint via 04-PRE-PLAN-NOTES.md.

3. **embedder.rs first-run state**: PROJECT.md says "silent until HF cache hit".
   VERIFIED PARTIALLY FALSE: `embedder.rs:67-71` already has 1 `eprintln!` start prompt; `:73-74` already has 1 line `.context()` failure message.
   SPEC IMPACT: requirement is "add progress indicator + failure recovery link", NOT "add first-run messaging from zero".

---

## 5 Inferred Requirements (Round 1 boundary derived)

| # | Label | Touchpoint |
|---|-------|------------|
| R1 | HF revision pin | `embedder.rs` const + `docs/ARCHITECTURE.md` §9.8 row |
| R2 | First-run download UX | `embedder.rs` start prompt + failure link |
| R3 | Offline / Clash-down recovery doc | `docs/embedder-offline-bootstrap.md` + `README.md` link |
| R4 | P2 `server.rs` A2A Index handler resilience | `server.rs:198` consecutive_fails counter |
| R5 | P2 `search.rs` Query path resilience | `search.rs:31`砍 5-attempt retry storm |

---

## Q4 Acceptance Criteria (Round 2 partial, LOCKED)

### R1 -- HF revision pin -- (a)+(b)+(c) all required

- (a) `grep -E 'const QWEN3_REVISION = "[a-f0-9]{40}"' experiments/poc-retrieval/src/embedder.rs` -> 1 hit
- (b) `grep '<revision-sha>' docs/ARCHITECTURE.md` (§9.8 history row 同一 sha) -> 1 hit
- (c) **Reload test**: delete `~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/<sha>/` -> rerun `index` -> sha subdir reborn matching source const -> exit 0

### R2 -- first-run download UX -- (a)+(b) only, (c) deferred

- (a) `grep -nE 'first-run download|huggingface\.co' embedder.rs` -> >=1 hit (URL + size + ETA wording)
- (b) `grep -n 'embedder-offline-bootstrap' embedder.rs` -> >=1 hit (failure path link)
- (c) PROGRESS INDICATOR DEFERRED: fastembed `Qwen3TextEmbedding::from_hf` 不暴露 progress callback。若 plan-phase 走 hf-hub 自定 fetch 路径 (R1 acceptance (c) reload test 也需要可控 cache layout 推荐这条路径), progress 指示作为副产物再升级 R2 加 (c)。

### R3 -- offline recovery doc -- (a)+(b)+(c) all required

- (a) `test -f docs/embedder-offline-bootstrap.md` -> file exists
- (b) `grep -cE '^## ' docs/embedder-offline-bootstrap.md` -> >=4 sections, must include: "Manual download" / "HF_HOME pre-seeding" / "HF_HUB_OFFLINE mode" / "Clash-China-down recovery"
- (c) `grep -n 'embedder-offline-bootstrap' README.md` -> >=1 hit (Quick start section link)

---

## OUTSTANDING (resume here)

### Q5 -- P2 resilience acceptance shape

**A.** EmbedError enum (Transient/Permanent/Timeout) 进 SPEC, R4/R5 基于 enum 三 arm 路由
**B (Claude recommends).** enum 不进 SPEC. R4 = mechanical patch复制 consecutive_fails counter from `main.rs:Index` to `server.rs:198` Index handler (15-30min). R5 = mechanical patch search.rs Query MAX_ATTEMPTS 5->2 + delete backoff sleep, single-fail returns clean Err immediately
**C.** Only R4 (server.rs same risk profile as main.rs Index), R5 deferred to P3

**Reasoning for B:** A 等于把 enum 设计 + 多 caller arm-match 工作量背在这刀里, scope 爆; C 不动 search.rs 让 Query 失败 wait 7.75s 留着, P3 推后 = 推到永远. B 是最小 P2 patch 兑现.

### Q6 -- E2E smoke as acceptance

**A.** 5 R 独立 PASS, no E2E
**B (Claude recommends).** 5 R + 1 E2E smoke: clean machine OR `rm -rf ~/.cache/huggingface/` -> `./codenexus query "x"` 观察 (1) download prompt (2) ETA/progress (3) query 出结果 -> 防火墙阻 huggingface.co -> rerun -> 观察 (4) failure prompt 含 recovery link
**C.** A + lightweight smoke (only cache-delete reload, no offline-failure verify)

**Reasoning for B:** Curry feedback rule 37 (open-source first-run UX is P1) 核心 = "用户在 chaos 环境下走过来"; 5 R grep PASS != "用户能用"; verifier 10min 跑 E2E = P1 directive 最低成本兑现.

---

## Current Ambiguity Score (post-Q4, pre-Q5/Q6)

```
Goal Clarity:        0.85 (min 0.75) PASS
Boundary Clarity:    0.85 (min 0.70) PASS
Constraint Clarity:  0.78 (min 0.65) PASS  (R1(c) reload test 钉 hf-cache layout)
Acceptance Criteria: 0.65 (min 0.70) FAIL (R4/R5/E2E 待答, 差 0.05)
Ambiguity: 0.204     (gate: <=0.20)  -- 差 0.004
```

Q5=B + Q6=B 后 Acceptance 期望 -> 0.80, ambiguity -> 0.18, gate PASS.

---

## 04-PRE-PLAN-NOTES.md Draft (NON-LOCKING planner hints, write next session alongside SPEC)

```
# Phase 4 First Slice -- Pre-Plan Notes (NON-LOCKING)

Purpose: planner hints. NOT requirements (those live in 04-SPEC.md).
SPEC is the contract; this file is signal that planner can ignore.

## HF revision-fetch path evaluation

Context: SPEC R1 locks "revision MUST be pinned" but does NOT lock implementation path.
fastembed-rs 5.13 `from_hf(repo, device, dtype, max_len)` 4-arg signature does not expose
revision (verified via `D:/dev-cache/.cargo/registry/src/.../fastembed-5.13.3/src/models/qwen3.rs`
per Phase 03.6 03.6-01-SUMMARY.md L153-188).

Two libraries for planner to `cargo doc` and pick:

- **hf-hub** (mature, community-widely-used): `ApiBuilder` + `Repo::with_revision`
  + `repo.get(filename)` -> local path. Snapshot layout: `~/.cache/huggingface/hub/
  models--<repo>/snapshots/<sha>/`. Used by fastembed internally.

- **huggingface_hub_rust** (NEW, 2026-04-09 official HF release): API surface closer
  to Python `huggingface_hub`. May expose `snapshot_download(revision=...)` directly.
  Worth 5-min `cargo doc` look before committing to hf-hub path.

Decision criteria:
1. If huggingface_hub_rust exposes clean `snapshot_download(repo, revision) -> path`
   -> prefer it (Python-convention parity, future-proof).
2. Else fall back to hf-hub `Repo::with_revision` + enumerated `repo.get(<file>)` for
   config.json, model.safetensors, tokenizer.json.
3. Either way: pass the resulting local snapshot path to `Qwen3TextEmbedding::from_hf`
   (it accepts repo-id-or-local-path).

Anti-pattern: do NOT fork fastembed-rs to add revision support. Wrapper layer (snapshot
fetch -> from_hf with local path) is cleaner separation.

## Progress indicator (R2 (c) deferred)

If R1 plan picks hf-hub or huggingface_hub_rust custom fetch (not fastembed default which
hides progress), the fetch loop exposes callback hooks for periodic `eprintln!` progress.
SPEC R2 only requires (a) start prompt + (b) failure link, but if implementation path
naturally exposes progress, deliver it -- and update R2 acceptance to include (c).
```

---

## Files to write next session (after Q5/Q6)

```
.planning/phases/codenexus-04-parity/04-SPEC.md         (locked requirements, gsd template)
.planning/phases/codenexus-04-parity/04-PRE-PLAN-NOTES.md  (planner hints, non-locking)
```

Then commit (gsd-spec-phase Step 7):
```
git add .planning/phases/codenexus-04-parity/{04-SPEC.md,04-PRE-PLAN-NOTES.md,INTERVIEW_LOG.md}
git commit -m "spec(phase-4): first slice SPEC -- first-run UX cluster + P2 resilience same-crate"
```

---

## Why this checkpoint exists

User shared Jina API keys mid-Round-2 (Q4 just answered, Q5/Q6 outstanding). Keys persisted to HKCU\Environment + api-key-inventory.md sec 9. User said Jina has official CLI to install + official MCP details next conversation. SPEC interview state captured here so next session can resume without re-asking Round 1 + Q4. Comprehensive on purpose -- checkpoint should be a complete handoff doc, not a teaser.

## Addendum 2026-04-28: Jina CLI install + env-inheritance pitfall

- **jina-cli installed**: `uv tool install jina-cli` (already present in uv tool env, version 0.1.0). Commands: bibtex/classify/datetime/dedup/embed/expand/grep/pdf/primer/read/rerank/screenshot/search.
- **Real API smoke PASS**: `jina embed "hello world" --json` returned `{object: "embedding", index: 0, embedding: [...]}` 1024-dim vector. JINA_API_KEY valid.
- **PITFALL (Windows, important for this session and future)**: `setx JINA_API_KEY <val>` writes to HKCU\Environment registry, but **already-running processes do NOT inherit the new env**. Claude Code's PowerShell subprocess inherits from Claude Code parent process, which captured env at launch (BEFORE setx). So `$env:JINA_API_KEY` reads len=0 in fresh tool-call PS, while `[Environment]::GetEnvironmentVariable('JINA_API_KEY','User')` reads len=65 from registry.
  - **Workaround in this session**: each PS tool-call that uses jina must hydrate first: `$env:JINA_API_KEY = [Environment]::GetEnvironmentVariable('JINA_API_KEY','User')` then call `jina ...`.
  - **Permanent fix**: restart Claude Code (parent process); all child PS subprocesses then inherit fresh env naturally without inline hydration.
- **JINA_READER_KEY usage with CLI**: jina CLI only reads `JINA_API_KEY` env name. To use Reader/Search with the user's separate Reader key, must either (a) swap env: `$env:JINA_API_KEY = $env:JINA_READER_KEY` before `jina read/search`, or (b) pass `--api-key $env:JINA_READER_KEY` flag (both `read` and `search` subcommands accept `--api-key TEXT`).
- **MCP**: official `jina-ai/MCP` repo identified (https://github.com/jina-ai/MCP, "Official Jina AI Remote MCP Server"). User said next conversation will give details + use mcp-setup skill to register.
