#!/usr/bin/env bash
# r4b_probe.sh -- standalone R4.b synthetic-failure A2A IndexRepo probe.
#
# Lifts R4.b gate from DEFERRED (per 04-04-FOLLOWUP-SUMMARY) to PASS by
# exercising 04-02 R4 server.rs `consecutive_fails` counter via the release
# binary, isolated from the e2e_first_run_smoke.sh harness fresh-install
# phases (which still hit the residual hf-hub Windows download wall).
#
# Mechanism:
#   1. Spawn `serve` with CODENEXUS_EMBED_FAIL=always so every embed Errs.
#   2. Submit A2A IndexRepo task via POST /tasks/send (max_consecutive_fail=5).
#   3. Poll /tasks/{id} until state=failed.
#   4. Assert: failed state reached, error msg contains "consecutive embed
#      failures", elapsed <60s.
#
# Reuses pre-seeded HF cache + existing release binary + existing poc.db.
# No fresh-install download path exercised.

set -euo pipefail
cd "$(dirname "$0")/.."

BIN="./target/release/codenexus-core.exe"
DB="./poc.db"
PORT=9897
TEST_REPO="D:/projects/obsidian-llm-wiki"
LOG_DIR="./eval/r4b_logs"

mkdir -p "$LOG_DIR"

[[ -x "$BIN"  ]] || { echo "FAIL: missing binary: $BIN"; exit 10; }
[[ -f "$DB"   ]] || { echo "FAIL: missing DB: $DB"; exit 11; }
[[ -d "$TEST_REPO" ]] || { echo "FAIL: missing TEST_REPO: $TEST_REPO"; exit 12; }

# JSON helper -- jq is not installed on this host, use Python 3.14
jget() {
  local f="$1" key="$2"
  python -c "import json,sys; d=json.load(open(r'$f')); print(d.get('$key',''))"
}

# Spawn server with fault injection
echo "[R4.b] spawning serve --port $PORT --db $DB with CODENEXUS_EMBED_FAIL=always"
CODENEXUS_EMBED_FAIL=always "$BIN" serve --port "$PORT" --db "$DB" \
  > "$LOG_DIR/serve.log" 2>&1 &
SERVE_PID=$!

cleanup() {
  set +e
  [[ -n "${SERVE_PID:-}" ]] && kill "$SERVE_PID" 2>/dev/null
  wait "$SERVE_PID" 2>/dev/null
  unset CODENEXUS_EMBED_FAIL
}
trap cleanup EXIT

# Wait for server ready (up to 30s -- first call loads embedder from cache)
for i in $(seq 1 30); do
  if curl -fsS "http://127.0.0.1:$PORT/healthz" > /dev/null 2>&1; then
    echo "[R4.b] server ready after ${i}s"
    break
  fi
  sleep 1
done
if ! curl -fsS "http://127.0.0.1:$PORT/healthz" > /dev/null 2>&1; then
  echo "FAIL: server did not become ready within 30s"
  /usr/bin/cat "$LOG_DIR/serve.log"
  exit 13
fi

# Submit IndexRepo task
SUBMIT_OUT="$LOG_DIR/submit.json"
START=$(date +%s)
curl -fsS -X POST "http://127.0.0.1:$PORT/tasks/send" \
  -H 'Content-Type: application/json' \
  -d "{\"operation\":{\"index_repo\":{\"repo\":\"$TEST_REPO\",\"max_consecutive_fail\":5}}}" \
  > "$SUBMIT_OUT"
TASK_ID="$(jget "$SUBMIT_OUT" id)"
if [[ -z "$TASK_ID" ]]; then
  echo "FAIL: task submit returned no id"
  /usr/bin/cat "$SUBMIT_OUT"
  exit 14
fi
echo "[R4.b] submitted task $TASK_ID"

# Poll until failed (60s budget; each consecutive iter ~7.75s, 5 iters ~39s)
POLL_OUT="$LOG_DIR/poll.json"
LAST_STATE=""
for i in $(seq 1 60); do
  curl -fsS "http://127.0.0.1:$PORT/tasks/$TASK_ID" > "$POLL_OUT"
  STATE="$(jget "$POLL_OUT" state)"
  LAST_STATE="$STATE"
  if [[ "$STATE" == "failed" ]]; then
    ELAPSED=$(($(date +%s) - START))
    ERR="$(jget "$POLL_OUT" error)"
    echo "[R4.b] state=failed elapsed=${ELAPSED}s"
    echo "[R4.b] error=$ERR"
    if [[ "$ERR" == *"consecutive embed failures"* ]] || [[ "$ERR" == *"consecutive=5/5"* ]]; then
      if [[ "$ELAPSED" -lt 60 ]]; then
        echo "PASS: R4.b -- A2A failed state with consecutive count in ${ELAPSED}s"
        exit 0
      fi
      echo "FAIL: failed state reached but elapsed ${ELAPSED}s >= 60s budget"
      exit 15
    fi
    echo "FAIL: failed state reached but error msg lacks 'consecutive': $ERR"
    exit 16
  fi
  sleep 1
done
echo "FAIL: task did not transition to failed within 60s (last state=$LAST_STATE)"
/usr/bin/cat "$LOG_DIR/serve.log"
exit 17
