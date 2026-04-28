#!/usr/bin/env bash
# E2E first-run smoke harness for Phase 4 first slice (Q6=B locked).
# Phase 4 first slice REVIEWS.md v2 -- HIGH#3 + HIGH#4 fixes applied:
#   - Uses Rust binary directly (NOT broken Go CLI signature)
#   - Isolated HF_HOME=$(mktemp -d) (NOT user's normal cache)
#   - R4.b + R5.b synthetic-failure tests via CODENEXUS_EMBED_FAIL
#
# Covers acceptance gates that require runtime evidence:
#   R1.c reload test  : pinned SHA reborn after cache delete
#   R1.d offline probe : load succeeds with HF_HUB_OFFLINE=1 + refs/main deleted
#   R2.a/b messaging  : start prompt has URL+ETA, failure path links recovery doc
#   R2.c progress     : >= 2 percentage milestones during model.safetensors download
#   R3 recovery link  : failure message contains 'embedder-offline-bootstrap'
#   E2E (1)-(6)       : SPEC Acceptance Criteria E2E smoke section
#   R4.b synthetic    : A2A IndexRepo failed state with consecutive count
#   R5.b synthetic    : query failure < 1.0s wall clock
#
# Portability note: assumes Git Bash on Windows OR bash/zsh on Unix. PowerShell
# users invoke via `bash experiments/poc-retrieval/eval/e2e_first_run_smoke.sh`.
# Required tools: bash, mktemp, curl, jq, awk, grep, ls, rm, mkdir, kill,
# /usr/bin/cat (NOT plain `cat` -- git-bash aliases cat to bat per pitfall).

set -u   # NOT -e -- capture exit codes for failure-path phases

REPO_ROOT="$(git rev-parse --show-toplevel)"
POC_DIR="$REPO_ROOT/experiments/poc-retrieval"

# Detect platform for binary extension
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
  BIN="$POC_DIR/target/release/codenexus-core.exe"
else
  BIN="$POC_DIR/target/release/codenexus-core"
fi

PINNED_SHA="97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3"
LOG="$POC_DIR/eval/e2e_first_run_smoke.log"
SERVE_PORT=9897   # arbitrary port for R4.b A2A test, avoid 9876 default

# Isolated HF_HOME -- HIGH#3 morning fix. User's normal cache UNTOUCHED.
# NOTE: on Windows/MSYS2 both `mktemp -d` and `mktemp -d -t prefix` resolve to
# /tmp which is MSYS2 tmpfs -- symlink creation for hf-hub cache layout FAILS
# there. hf-hub snapshots/<sha>/ uses POSIX symlinks pointing to blobs; this
# requires an NTFS path. Use Windows %LOCALAPPDATA%\Temp explicitly.
# Verified 2026-04-28: NTFS supports hf-hub's POSIX symlinks. C: is too tight
# (~27GB free; 1.2GB download caused ERROR_DISK_FULL os error 112 on first run).
# D: has 216GB free; D:/temp is same NTFS so symlinks work identically.
# D:/temp is created externally if missing; we mkdir -p as defensive idempotency.
_WINTMP="D:/temp"
mkdir -p "$_WINTMP"
HF_HOME="${_WINTMP}/codenexus-e2e-$$"
mkdir -p "$HF_HOME"
export HF_HOME

# CRITICAL: hf-hub sync.rs:904 uses std::env::temp_dir() for the .incomplete
# download tempfile (1.2 GB). Rust's temp_dir() honors Windows TMP/TEMP env vars.
# Without this override, hf-hub writes to C:\Users\...\AppData\Local\Temp which
# may have insufficient space (we hit ERROR_DISK_FULL os error 112 on first run
# when C: had only ~27GB free). Force download tempfiles to land on D: too.
export TMP="$_WINTMP"
export TEMP="$_WINTMP"
export TMPDIR="$_WINTMP"

# Test DB lives inside isolated HF_HOME so it is also cleaned up on exit
TEST_DB="$HF_HOME/e2e_smoke.db"
# obsidian-llm-wiki has TypeScript files -- the parser is TS-only (tree-sitter-typescript).
# poc-retrieval itself is Rust, produces 0 symbols. Use the TS project that poc.db was built from.
TEST_REPO="D:/projects/obsidian-llm-wiki"

# Pre-seed isolated HF_HOME from real cache -- avoids downloading the model twice.
# The pre-index phase uses the pre-seeded cache (fast); Phase 1 deletes it to force
# a real re-download (exercising R2.a messaging + R2.c progress milestones).
# Uses python3 shutil.copytree(symlinks=False) to dereference Windows junction-
# symlinks in snapshots/<sha>/ -- cp -r and rsync both fail on MSYS2 with these.
REAL_HF_CACHE="$HOME/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B"
if [ -d "$REAL_HF_CACHE/snapshots/$PINNED_SHA" ]; then
  echo "[setup] pre-seeding isolated HF_HOME from real cache via python3 shutil (dereferences Windows symlinks)" | tee -a "$LOG"
  mkdir -p "$HF_HOME/hub"
  python3 -c "
import shutil, os

def ignore_partials(directory, contents):
    # Exclude .part and .lock files -- these are in-progress download artifacts.
    # If copied to isolated HF_HOME, hf-hub tries to resume them from network
    # (causing the 49% stall + os error 112 failure seen in earlier harness runs).
    return [f for f in contents if f.endswith('.part') or f.endswith('.lock')]

src = os.path.expanduser('~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B')
dst = os.path.join(os.environ['HF_HOME'], 'hub', 'models--Qwen--Qwen3-Embedding-0.6B')
shutil.copytree(src, dst, symlinks=False, dirs_exist_ok=True, ignore=ignore_partials)
" 2>&1 | tee -a "$LOG"
  if [ -f "$HF_HOME/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/$PINNED_SHA/config.json" ]; then
    echo "[setup] pre-seed done: model files present in isolated HF_HOME" | tee -a "$LOG"
  else
    echo "[setup] pre-seed FAILED -- pre-index will download from network (may be slow)" | tee -a "$LOG"
  fi
else
  echo "[setup] real cache missing pinned SHA -- pre-index will download from network" | tee -a "$LOG"
fi

# Idempotent cleanup at exit (covers HTTPS_PROXY leak + isolated HF_HOME)
cleanup() {
  set +e
  unset HTTPS_PROXY HTTP_PROXY HF_HUB_OFFLINE CODENEXUS_EMBED_FAIL
  if [[ -n "${SERVE_PID:-}" ]]; then
    kill "$SERVE_PID" 2>/dev/null
    wait "$SERVE_PID" 2>/dev/null
  fi
  rm -rf "$HF_HOME"
}
trap cleanup EXIT INT TERM

echo "==[ Phase 4 E2E smoke harness v2 ]==" | tee "$LOG"
echo "Pinned SHA: $PINNED_SHA" | tee -a "$LOG"
echo "Isolated HF_HOME: $HF_HOME" | tee -a "$LOG"
echo "Binary: $BIN" | tee -a "$LOG"
date | tee -a "$LOG"

# ---- Phase 0: ensure release artifact exists (Plan 04-00 rename verification) ----
if [ ! -x "$BIN" ]; then
  echo "[setup] $BIN missing -- running 'make build' to verify Plan 04-00 rename" | tee -a "$LOG"
  ( cd "$REPO_ROOT" && make build ) >> "$LOG" 2>&1 || {
    echo "FAIL: make build (Plan 04-00 binary rename may be incomplete)" | tee -a "$LOG"
    exit 10
  }
fi
if [ ! -x "$BIN" ]; then
  echo "FAIL: $BIN still missing after make build" | tee -a "$LOG"
  exit 11
fi

# ---- Phase 0b: pre-index inside isolated HF_HOME ----
# Pre-index runs INSIDE isolated HF_HOME so when we delete the snapshot
# in Phase 1, the test DB is preserved but the model cache is cleared.
# (HIGH#3 morning fix -- pre-index ordering bug fixed.)
echo "[setup] indexing $TEST_REPO into $TEST_DB (uses isolated HF_HOME=$HF_HOME, HF_HUB_OFFLINE=1)" | tee -a "$LOG"
# HF_HUB_OFFLINE=1: use pre-seeded isolated cache -- no network access for pre-index.
# Phase 1 (clean cache) is where the actual download + R2.a/R2.c test happens.
HF_HUB_OFFLINE=1 "$BIN" index --repo "$TEST_REPO" --db "$TEST_DB" --max-consecutive-fail 5 \
  >> "$LOG" 2>&1 || {
    echo "FAIL: pre-index step" | tee -a "$LOG"
    exit 12
  }

# Verify the pre-index produced a non-empty DB (E2E LOW fix -- query
# returning empty results would let the gate pass trivially)
# Use python3 via env var -- sqlite3 CLI not in PATH on this Windows machine.
# Pass path via DB_PATH env var to avoid Windows backslash escape issues.
INDEXED_COUNT=$(DB_PATH="$TEST_DB" python3 -c "
import sqlite3, os, sys
db = os.environ['DB_PATH']
try:
    conn = sqlite3.connect(db)
    cnt = conn.execute('SELECT COUNT(*) FROM symbols').fetchone()[0]
    print(cnt)
    conn.close()
except Exception as e:
    print(0)
" 2>/dev/null || echo 0)
if [ "$INDEXED_COUNT" -lt 1 ]; then
  echo "FAIL: pre-index produced 0 symbols (need >=1 for meaningful query gate)" | tee -a "$LOG"
  exit 13
fi
echo "[setup] pre-index ok: $INDEXED_COUNT symbols" | tee -a "$LOG"

HF_CACHE_DIR="$HF_HOME/hub/models--Qwen--Qwen3-Embedding-0.6B"

# ---- E2E Phase 1: clean-cache success path ----
echo "" | tee -a "$LOG"
echo "==[ E2E (1)-(3): clean cache + working network ]==" | tee -a "$LOG"
if [ -d "$HF_CACHE_DIR/snapshots/$PINNED_SHA" ]; then
  echo "[phase 1] removing entire model dir from isolated HF_HOME (Windows-junction-safe)" | tee -a "$LOG"
  # W2 fix (rerun-1303 checker): on Windows hf-hub may use junction-copies instead of
  # POSIX symlinks for snapshots/<sha>/. Deleting only snapshots+blobs would leave
  # the snapshot-dir file copies intact, causing repo.get() to skip re-download and
  # R2.c progress milestones to never fire. Nuking the whole $HF_CACHE_DIR guarantees
  # re-download regardless of OS layout. Safe because $HF_CACHE_DIR is inside the
  # isolated $HF_HOME=$(mktemp -d) sandbox -- never touches the user's ~/.cache.
  rm -rf "$HF_CACHE_DIR"
fi

PHASE1_OUT="$HF_HOME/.e2e_phase1.txt"
"$BIN" query "ObsidianAdapter" --db "$TEST_DB" > "$PHASE1_OUT" 2>&1
PHASE1_EXIT=$?

echo "[phase 1] exit=$PHASE1_EXIT" | tee -a "$LOG"
/usr/bin/cat "$PHASE1_OUT" | tee -a "$LOG"

# E2E (1): start prompt with URL + ETA appears
if grep -qE 'first-run download.*huggingface\.co' "$PHASE1_OUT" \
   && grep -qE '30-60s|broadband' "$PHASE1_OUT"; then
  echo "PASS: E2E (1) start prompt URL+ETA visible" | tee -a "$LOG"
else
  echo "FAIL: E2E (1) start prompt missing URL or ETA wording" | tee -a "$LOG"
  exit 21
fi

# E2E (1b): R2.c progress milestones -- at least 2 percentage lines
PROGRESS_HITS=$(grep -cE 'downloading model: [0-9]+%' "$PHASE1_OUT" || true)
if [ "$PROGRESS_HITS" -ge 2 ]; then
  echo "PASS: E2E (1b) R2.c progress >=2 milestones ($PROGRESS_HITS lines)" | tee -a "$LOG"
else
  echo "FAIL: E2E (1b) R2.c progress milestones = $PROGRESS_HITS (need >=2)" | tee -a "$LOG"
  exit 27
fi

# E2E (2): download completes -- proxy: snapshot dir reborn at pinned SHA
if [ -d "$HF_CACHE_DIR/snapshots/$PINNED_SHA" ]; then
  echo "PASS: E2E (2) snapshot dir reborn at pinned SHA = R1.c reload test" | tee -a "$LOG"
else
  echo "FAIL: E2E (2) snapshot dir not reborn at $PINNED_SHA" | tee -a "$LOG"
  ls -la "$HF_CACHE_DIR/snapshots/" 2>&1 | tee -a "$LOG"
  exit 22
fi

# E2E (3): query exits 0
if [ "$PHASE1_EXIT" -eq 0 ]; then
  echo "PASS: E2E (3) query exit 0" | tee -a "$LOG"
else
  echo "FAIL: E2E (3) query exit $PHASE1_EXIT" | tee -a "$LOG"
  exit 23
fi

# ---- E2E Phase 1c: R1.d offline-mode probe ----
# With cache populated, set HF_HUB_OFFLINE=1 + delete refs/main.
# Load MUST still succeed because Repo::with_revision uses snapshot dir directly.
echo "" | tee -a "$LOG"
echo "==[ E2E (1c) R1.d: offline-mode probe ]==" | tee -a "$LOG"
rm -f "$HF_CACHE_DIR/refs/main"
PHASE1C_OUT="$HF_HOME/.e2e_phase1c.txt"
HF_HUB_OFFLINE=1 "$BIN" query "ObsidianAdapter" --db "$TEST_DB" > "$PHASE1C_OUT" 2>&1
PHASE1C_EXIT=$?
unset HF_HUB_OFFLINE

if [ "$PHASE1C_EXIT" -eq 0 ]; then
  echo "PASS: E2E (1c) R1.d offline-mode load succeeded -- pin is FUNCTIONAL" | tee -a "$LOG"
else
  echo "FAIL: E2E (1c) R1.d offline-mode load failed (exit $PHASE1C_EXIT) -- pin is decorative" | tee -a "$LOG"
  /usr/bin/cat "$PHASE1C_OUT" | tee -a "$LOG"
  exit 28
fi

# ---- E2E Phase 2: network-blocked failure path ----
echo "" | tee -a "$LOG"
echo "==[ E2E (4)-(5): network blocked via HTTPS_PROXY ]==" | tee -a "$LOG"
echo "[phase 2] removing entire model dir to force fresh download (Windows-junction-safe)" | tee -a "$LOG"
# W2 fix (rerun-1303 checker): same junction-safety reasoning as Phase 1.
rm -rf "$HF_CACHE_DIR"

PHASE2_OUT="$HF_HOME/.e2e_phase2.txt"
HTTPS_PROXY=http://0.0.0.0:1 HTTP_PROXY=http://0.0.0.0:1 \
  "$BIN" query "ObsidianAdapter" --db "$TEST_DB" > "$PHASE2_OUT" 2>&1
PHASE2_EXIT=$?

echo "[phase 2] exit=$PHASE2_EXIT" | tee -a "$LOG"
/usr/bin/cat "$PHASE2_OUT" | tee -a "$LOG"

# E2E (4): failure message contains link to docs/embedder-offline-bootstrap.md
if grep -qF 'embedder-offline-bootstrap' "$PHASE2_OUT"; then
  echo "PASS: E2E (4) failure message links to recovery doc" | tee -a "$LOG"
else
  echo "FAIL: E2E (4) failure message missing recovery doc link" | tee -a "$LOG"
  exit 24
fi

# E2E (5): exit non-zero
if [ "$PHASE2_EXIT" -ne 0 ]; then
  echo "PASS: E2E (5) exit non-zero ($PHASE2_EXIT)" | tee -a "$LOG"
else
  echo "FAIL: E2E (5) expected non-zero exit, got 0" | tee -a "$LOG"
  exit 25
fi

# ---- E2E Phase 3: network restored, re-success ----
echo "" | tee -a "$LOG"
echo "==[ E2E (6): network restored, re-success ]==" | tee -a "$LOG"
unset HTTPS_PROXY HTTP_PROXY

PHASE3_OUT="$HF_HOME/.e2e_phase3.txt"
"$BIN" query "ObsidianAdapter" --db "$TEST_DB" > "$PHASE3_OUT" 2>&1
PHASE3_EXIT=$?

echo "[phase 3] exit=$PHASE3_EXIT" | tee -a "$LOG"
/usr/bin/cat "$PHASE3_OUT" | tee -a "$LOG"

if [ "$PHASE3_EXIT" -eq 0 ] \
   && [ -d "$HF_CACHE_DIR/snapshots/$PINNED_SHA" ] \
   && grep -qE 'first-run download' "$PHASE3_OUT"; then
  echo "PASS: E2E (6) restored network re-succeeds with same SHA reborn" | tee -a "$LOG"
else
  echo "FAIL: E2E (6)" | tee -a "$LOG"
  exit 26
fi

# ---- E2E Phase 4: R4.b synthetic -- A2A IndexRepo with consecutive_fails ----
echo "" | tee -a "$LOG"
echo "==[ R4.b: A2A IndexRepo synthetic-failure test ]==" | tee -a "$LOG"

# Spawn server with fault injection in background
CODENEXUS_EMBED_FAIL=always "$BIN" serve --port "$SERVE_PORT" --db "$TEST_DB" \
  > "$HF_HOME/.serve.log" 2>&1 &
SERVE_PID=$!

# Wait for server to be ready (up to 10s)
for i in 1 2 3 4 5 6 7 8 9 10; do
  if curl -fsS "http://127.0.0.1:$SERVE_PORT/healthz" > /dev/null 2>&1; then
    break
  fi
  sleep 1
done
if ! curl -fsS "http://127.0.0.1:$SERVE_PORT/healthz" > /dev/null 2>&1; then
  echo "FAIL: R4.b -- server failed to start" | tee -a "$LOG"
  /usr/bin/cat "$HF_HOME/.serve.log" | tee -a "$LOG"
  exit 41
fi
echo "[R4.b] server ready on :$SERVE_PORT with CODENEXUS_EMBED_FAIL=always" | tee -a "$LOG"

# Submit IndexRepo task
SUBMIT_OUT="$HF_HOME/.r4b_submit.json"
curl -fsS -X POST "http://127.0.0.1:$SERVE_PORT/tasks/send" \
  -H 'Content-Type: application/json' \
  -d "{\"operation\":{\"index_repo\":{\"repo\":\"$TEST_REPO\",\"max_consecutive_fail\":5}}}" \
  > "$SUBMIT_OUT" 2>&1
TASK_ID=$(jq -r '.id' "$SUBMIT_OUT")
if [[ -z "$TASK_ID" || "$TASK_ID" == "null" ]]; then
  echo "FAIL: R4.b -- task submit returned no id" | tee -a "$LOG"
  /usr/bin/cat "$SUBMIT_OUT" | tee -a "$LOG"
  exit 42
fi
echo "[R4.b] submitted task $TASK_ID at $(date +%s)" | tee -a "$LOG"

# Poll until failed (or 60s timeout -- budget is ~5x7.75s ~= 39s)
START=$(date +%s)
for i in $(seq 1 60); do
  POLL_OUT="$HF_HOME/.r4b_poll.json"
  curl -fsS "http://127.0.0.1:$SERVE_PORT/tasks/$TASK_ID" > "$POLL_OUT" 2>&1
  STATE=$(jq -r '.state' "$POLL_OUT")
  if [[ "$STATE" == "failed" ]]; then
    ELAPSED=$(($(date +%s) - START))
    ERR_MSG=$(jq -r '.error' "$POLL_OUT")
    echo "[R4.b] state=failed after ${ELAPSED}s, error=$ERR_MSG" | tee -a "$LOG"
    if [[ "$ERR_MSG" =~ consecutive=5/5 ]] || [[ "$ERR_MSG" =~ "consecutive embed failures" ]]; then
      if [ "$ELAPSED" -lt 60 ]; then
        echo "PASS: R4.b -- A2A failed state with consecutive count in <60s ($ELAPSED s)" | tee -a "$LOG"
      else
        echo "FAIL: R4.b -- failed state reached but elapsed ${ELAPSED}s exceeds 60s budget" | tee -a "$LOG"
        exit 43
      fi
    else
      echo "FAIL: R4.b -- failed state reached but error msg lacks 'consecutive' count: $ERR_MSG" | tee -a "$LOG"
      exit 44
    fi
    break
  fi
  if [ "$i" -eq 60 ]; then
    echo "FAIL: R4.b -- task did not transition to failed within 60s (state=$STATE)" | tee -a "$LOG"
    exit 45
  fi
  sleep 1
done

# Stop server
kill "$SERVE_PID" 2>/dev/null
wait "$SERVE_PID" 2>/dev/null
SERVE_PID=""
unset CODENEXUS_EMBED_FAIL

# ---- E2E Phase 5: R5.b synthetic -- Query path < 1.0s wall clock ----
echo "" | tee -a "$LOG"
echo "==[ R5.b: query synthetic-failure timing test ]==" | tee -a "$LOG"

R5B_OUT="$HF_HOME/.r5b.txt"
START_NS=$(date +%s%N 2>/dev/null || date +%s000000000)
CODENEXUS_EMBED_FAIL=always "$BIN" query "ObsidianAdapter" --db "$TEST_DB" \
  > "$R5B_OUT" 2>&1
R5B_EXIT=$?
END_NS=$(date +%s%N 2>/dev/null || date +%s000000000)
ELAPSED_MS=$(( (END_NS - START_NS) / 1000000 ))
unset CODENEXUS_EMBED_FAIL

echo "[R5.b] exit=$R5B_EXIT elapsed=${ELAPSED_MS}ms" | tee -a "$LOG"

if [ "$R5B_EXIT" -eq 0 ]; then
  echo "FAIL: R5.b -- expected non-zero exit (CODENEXUS_EMBED_FAIL=always) but got 0" | tee -a "$LOG"
  exit 51
fi
if [ "$ELAPSED_MS" -ge 1000 ]; then
  echo "FAIL: R5.b -- wall clock ${ELAPSED_MS}ms exceeds 1000ms budget" | tee -a "$LOG"
  exit 52
fi
echo "PASS: R5.b -- query failure in ${ELAPSED_MS}ms (< 1000ms budget)" | tee -a "$LOG"

echo "" | tee -a "$LOG"
echo "==[ ALL E2E + SYNTHETIC PHASES PASSED ]==" | tee -a "$LOG"
exit 0
