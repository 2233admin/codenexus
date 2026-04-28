#!/usr/bin/env bash
# preseed-hf-cache.sh -- Automate HF Hub cache pre-seeding for CodeNexus.
#
# Purpose: workaround for the hf-hub 0.5 Windows fresh-download bug
# (deterministic 49% / 567 MB wall, see PROJECT.md line 98). Copies an
# already-working HF cache (from another host or tarball) into the user's
# HF_HOME so `HF_HUB_OFFLINE=1 ./codenexus serve` runs offline.
#
# This is the canonical Windows install path documented in
# docs/embedder-offline-bootstrap.md. It does NOT fix the upstream bug;
# it bypasses the broken fresh-download path.
#
# Usage:
#   preseed-hf-cache.sh --source <dir-or-tarball> [options]
#   preseed-hf-cache.sh --verify-only [options]
#   preseed-hf-cache.sh --help
#
# Options:
#   --source <path>        Existing HF cache dir (any host) OR tarball (.tar / .tar.gz).
#                          Required unless --verify-only.
#   --target <path>        Target HF cache root. Default: ${HF_HOME:-$HOME/.cache/huggingface}/hub
#   --model <repo-id>      HF repo id (org/name). Default: Qwen/Qwen3-Embedding-0.6B
#   --revision <sha>       Pinned revision. Default: 97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3
#   --verify-only          Skip copy; only check target has snapshot/<sha>/model.safetensors > 1GB.
#   --help                 Show usage and exit 0.

set -euo pipefail

DEFAULT_MODEL="Qwen/Qwen3-Embedding-0.6B"
DEFAULT_REVISION="97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3"

show_help() {
  /usr/bin/cat <<'HELP'
preseed-hf-cache.sh -- Pre-seed HuggingFace Hub cache for CodeNexus.

Workaround for hf-hub 0.5 Windows fresh-download bug (49% / 567 MB wall).
Copies a working HF cache (from another host or tarball) into HF_HOME so
`HF_HUB_OFFLINE=1 ./codenexus serve` runs offline.

USAGE:
  preseed-hf-cache.sh --source <dir-or-tarball> [options]
  preseed-hf-cache.sh --verify-only [options]
  preseed-hf-cache.sh --help

OPTIONS:
  --source PATH       Existing HF cache (model dir) OR tarball (.tar/.tar.gz). Required unless --verify-only.
  --target PATH       Target HF hub root. Default: ${HF_HOME:-$HOME/.cache/huggingface}/hub
  --model REPO-ID     HF repo (org/name). Default: Qwen/Qwen3-Embedding-0.6B
  --revision SHA      Pinned revision. Default: 97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3
  --verify-only       Skip copy; check target snapshot/<sha>/model.safetensors > 1GB.
  --help              This message.

EXAMPLES:
  # From another host's HF cache (after rsync/scp)
  preseed-hf-cache.sh --source /shared/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B

  # From a tarball
  preseed-hf-cache.sh --source ~/Downloads/qwen3-cache.tar.gz

  # Verify only
  preseed-hf-cache.sh --verify-only

EXIT CODES:
  0  Success (copy complete OR verify passes)
  1  Generic failure (bad arg, copy error)
  2  Source missing or invalid layout
  3  Target verify failed (missing/empty model.safetensors)
HELP
}

# Parse args
SOURCE=""
TARGET="${HF_HOME:-$HOME/.cache/huggingface}/hub"
MODEL="$DEFAULT_MODEL"
REVISION="$DEFAULT_REVISION"
VERIFY_ONLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --source)        SOURCE="$2"; shift 2 ;;
    --target)        TARGET="$2"; shift 2 ;;
    --model)         MODEL="$2"; shift 2 ;;
    --revision)      REVISION="$2"; shift 2 ;;
    --verify-only)   VERIFY_ONLY=true; shift ;;
    --help|-h)       show_help; exit 0 ;;
    *)               echo "[preseed] ERROR: unknown arg: $1" >&2; show_help >&2; exit 1 ;;
  esac
done

# Derived: model dir name (HF convention: org/name -> models--org--name)
MODEL_DIR_NAME="models--$(echo "$MODEL" | sed 's|/|--|g')"
TARGET_MODEL_DIR="$TARGET/$MODEL_DIR_NAME"

verify_target() {
  local snap_dir="$TARGET_MODEL_DIR/snapshots/$REVISION"
  local safetensors="$snap_dir/model.safetensors"
  if [[ ! -d "$snap_dir" ]]; then
    echo "[preseed] VERIFY FAIL: snapshot dir missing: $snap_dir" >&2
    return 3
  fi
  if [[ ! -f "$safetensors" ]] && [[ ! -L "$safetensors" ]]; then
    echo "[preseed] VERIFY FAIL: model.safetensors missing in $snap_dir" >&2
    return 3
  fi
  # Resolve symlink if needed; check size > 1GB (model is ~1.19 GB)
  local resolved
  resolved="$(readlink -f "$safetensors" 2>/dev/null || echo "$safetensors")"
  local size_bytes
  size_bytes="$(stat -c%s "$resolved" 2>/dev/null || stat -f%z "$resolved" 2>/dev/null || wc -c < "$resolved")"
  if [[ -z "$size_bytes" ]] || [[ "$size_bytes" -lt 1000000000 ]]; then
    echo "[preseed] VERIFY FAIL: model.safetensors size=$size_bytes (expected > 1GB)" >&2
    return 3
  fi
  local size_mb=$((size_bytes / 1024 / 1024))
  echo "[preseed] verify OK: $TARGET_MODEL_DIR sha=$REVISION safetensors=${size_mb}MB"
  return 0
}

if [[ "$VERIFY_ONLY" == "true" ]]; then
  verify_target
  exit $?
fi

# --source required
if [[ -z "$SOURCE" ]]; then
  echo "[preseed] ERROR: --source required (or use --verify-only)" >&2
  show_help >&2
  exit 1
fi

# Validate source exists
if [[ ! -e "$SOURCE" ]]; then
  echo "[preseed] ERROR: source does not exist: $SOURCE" >&2
  exit 2
fi

# Ensure target parent exists
mkdir -p "$TARGET_MODEL_DIR"

# Tarball mode
if [[ "$SOURCE" == *.tar.gz ]] || [[ "$SOURCE" == *.tgz ]]; then
  echo "[preseed] extracting tarball $SOURCE -> $TARGET_MODEL_DIR (gzip)"
  tar -xzf "$SOURCE" -C "$TARGET_MODEL_DIR" --strip-components=0
elif [[ "$SOURCE" == *.tar ]]; then
  echo "[preseed] extracting tarball $SOURCE -> $TARGET_MODEL_DIR (plain)"
  tar -xf "$SOURCE" -C "$TARGET_MODEL_DIR" --strip-components=0
elif [[ -d "$SOURCE" ]]; then
  # Directory mode: bulk copy with symlink resolution.
  # cp -rL resolves symlinks so target gets real files (no dangling links
  # if blobs/ layout differs across hosts). Storage cost: blobs duplicated
  # under snapshots/, but acceptable as a workaround.
  echo "[preseed] copying $SOURCE -> $TARGET_MODEL_DIR (cp -rL)"
  # Use trailing /. so cp copies contents not the dir itself
  cp -rL "$SOURCE/." "$TARGET_MODEL_DIR/"
else
  echo "[preseed] ERROR: source must be a directory, .tar.gz, or .tar: $SOURCE" >&2
  exit 2
fi

# Verify after copy
verify_target
