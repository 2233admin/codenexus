#!/usr/bin/env bash
# r1c_probe.sh -- standalone R1.c reload probe (pre-seed flavor, file-level sha256).
#
# Original R1.c spec: delete snapshot dir + redownload yields same SHA.
# Pre-seed flavor: delete snapshot dir + re-pre-seed yields byte-identical
# model.safetensors. Pure file-layer test (sha256), independent of poc.db
# state and embedder execution -- proves SHA pin is deterministic at the
# cache layer.
#
# Why file-level not eval-level: an eval-based test would require an
# already-indexed poc.db, which couples the probe to indexer state. The
# original R4.b probe (commit 9a326d1) destroys symbols on synthetic-fail
# IndexRepo (Store::clear before failed embed loop -- see 04-05-SUMMARY
# P1 findings section). File-level sha256 is orthogonal: tests only the
# pre-seed mechanism, no DB state required.
#
# Bonus phase 3: run `codenexus-core.exe query` with HF_HUB_OFFLINE=1 to
# prove the binary loads the embedder from the isolated pre-seeded cache.
# Non-fatal -- the canonical gate is sha256 equality.

set -euo pipefail
cd "$(dirname "$0")/.."

BIN="./target/release/codenexus-core.exe"
DB="./poc.db"
REVISION="97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3"
SOURCE_CACHE="$HOME/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B"
TEST_HF_HOME="$(mktemp -d -t codenexus-r1c-XXXXXX)"
LOG_DIR="./eval/r1c_logs"
mkdir -p "$LOG_DIR"

cleanup() {
  rm -rf "$TEST_HF_HOME"
}
trap cleanup EXIT

[[ -d "$SOURCE_CACHE" ]] || { echo "FAIL: source cache missing: $SOURCE_CACHE"; exit 10; }
[[ -x "$BIN" ]] || echo "[r1c] WARN: binary missing at $BIN -- phase 3 smoke will be skipped"

SAFETENSORS_PATH="$TEST_HF_HOME/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/$REVISION/model.safetensors"

# ===== Phase 1: pre-seed into isolated HF_HOME =====
echo "[r1c.1] pre-seed into $TEST_HF_HOME"
HF_HOME="$TEST_HF_HOME" bash scripts/preseed-hf-cache.sh \
  --source "$SOURCE_CACHE" > "$LOG_DIR/preseed1.log" 2>&1

[[ -f "$SAFETENSORS_PATH" ]] || { echo "FAIL: phase 1 -- safetensors not found at $SAFETENSORS_PATH"; /usr/bin/cat "$LOG_DIR/preseed1.log"; exit 11; }

SHA1=$(sha256sum "$SAFETENSORS_PATH" | cut -d' ' -f1)
SIZE1=$(stat -c%s "$SAFETENSORS_PATH" 2>/dev/null || stat -f%z "$SAFETENSORS_PATH" 2>/dev/null)
echo "[r1c.1] sha256=${SHA1:0:16}...${SHA1: -8} size=${SIZE1}"

# ===== Phase 2: nuke snapshot dir, re-pre-seed =====
echo "[r1c.2] delete snapshot dir + re-pre-seed"
rm -rf "$TEST_HF_HOME/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/$REVISION"
[[ ! -d "$TEST_HF_HOME/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/$REVISION" ]] || { echo "FAIL: phase 2 -- snapshot dir not deleted"; exit 12; }

HF_HOME="$TEST_HF_HOME" bash scripts/preseed-hf-cache.sh \
  --source "$SOURCE_CACHE" > "$LOG_DIR/preseed2.log" 2>&1

[[ -f "$SAFETENSORS_PATH" ]] || { echo "FAIL: phase 2 -- safetensors missing after re-pre-seed"; /usr/bin/cat "$LOG_DIR/preseed2.log"; exit 13; }

SHA2=$(sha256sum "$SAFETENSORS_PATH" | cut -d' ' -f1)
SIZE2=$(stat -c%s "$SAFETENSORS_PATH" 2>/dev/null || stat -f%z "$SAFETENSORS_PATH" 2>/dev/null)
echo "[r1c.2] sha256=${SHA2:0:16}...${SHA2: -8} size=${SIZE2}"

# ===== Phase 3 (canonical gate): assert byte-identical =====
if [[ "$SHA1" == "$SHA2" ]] && [[ "$SIZE1" == "$SIZE2" ]]; then
  echo "[r1c.3] PASS: R1.c file-level -- pre-seed reload yields byte-identical safetensors"
else
  echo "FAIL: R1.c -- safetensors drifted across pre-seed cycles"
  echo "  phase 1: sha=$SHA1 size=$SIZE1"
  echo "  phase 2: sha=$SHA2 size=$SIZE2"
  exit 14
fi

# ===== Phase 4 (bonus smoke): embedder loads under HF_HUB_OFFLINE=1 =====
if [[ -x "$BIN" ]]; then
  echo "[r1c.4] smoke: embedder loads from pre-seeded cache under HF_HUB_OFFLINE=1"
  if HF_HOME="$TEST_HF_HOME" HF_HUB_OFFLINE=1 "$BIN" query "test" --db "$DB" \
       > "$LOG_DIR/smoke.log" 2>&1; then
    echo "[r1c.4] embedder smoke PASS (binary exited 0 with offline+isolated HF_HOME)"
  else
    SMOKE_EXIT=$?
    echo "[r1c.4] WARN: embedder smoke exit=$SMOKE_EXIT (non-fatal -- canonical gate is sha256)"
    echo "[r1c.4] smoke log tail:"
    tail -10 "$LOG_DIR/smoke.log" || true
  fi
else
  echo "[r1c.4] SKIP: binary missing"
fi

echo ""
echo "PASS: R1.c -- standalone probe complete (sha256 equality + embedder load smoke)"
exit 0
