# Embedder Offline Bootstrap

The CodeNexus embedder downloads ~1.2 GB of Qwen3-Embedding-0.6B
weights from `huggingface.co` on first run. If you are offline,
behind a restricted proxy, or your route to `huggingface.co` is
broken (e.g. Clash-China is down), use one of the four recovery
paths below. The pinned revision is documented in
`docs/ARCHITECTURE.md` §9.8 and lives as `QWEN3_REVISION` const in
`experiments/poc-retrieval/src/embedder.rs`.

> **Note:** CodeNexus pins the model at a specific HF Hub commit SHA
> and loads it via `Qwen3TextEmbedding::new` with local files (NOT
> `Qwen3TextEmbedding::from_hf`). Once
> `~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/<sha>/`
> exists, the embedder will load fully offline. The `refs/main` file in the
> cache is hf-hub-internal and does NOT need to be written manually
> -- only the `snapshots/<sha>/` directory matters.

## Manual safetensors download

Download the 9 files fastembed expects directly from the HuggingFace
web UI (or any mirror that speaks the HF Hub API), then place them
under the canonical cache layout:

```
~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/
  blobs/         # one file per content-addressed blob
  snapshots/<sha>/
    config.json                          # required (parsed as Qwen3Config)
    config_sentence_transformers.json
    tokenizer.json                       # required (Tokenizer::from_file)
    tokenizer_config.json
    vocab.json
    merges.txt
    model.safetensors                    # required (mmap by VarBuilder, ~600 MB)
    modules.json
    1_Pooling/config.json
```

Of the 9 files, only 3 are functionally required by the loader
(`config.json`, `tokenizer.json`, `model.safetensors`). The other 6
are kept for cache-layout completeness and possible future fastembed
versions.

Each file under `snapshots/<sha>/` is a symlink (or junction on
Windows) to a content-addressed blob in `blobs/`. If you cannot
create symlinks, copy the actual file content into the snapshot
directory directly -- hf-hub will accept this layout.

The `<sha>` value is the `QWEN3_REVISION` constant
(`97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3` as of writing).

## HF_HOME pre-seeding

If you have a working network on machine A and a broken network
on machine B, pre-seed the cache on A and copy:

```bash
# on machine A (working network):
./bin/codenexus query "any string"   # triggers download
tar -czf hf-cache.tgz -C ~/.cache/huggingface/hub \
  models--Qwen--Qwen3-Embedding-0.6B/

# on machine B (no network):
mkdir -p ~/.cache/huggingface/hub/
tar -xzf hf-cache.tgz -C ~/.cache/huggingface/hub/
./bin/codenexus query "any string"   # uses cache, no download
```

Or override the cache root globally with `HF_HOME=/path/to/cache
./bin/codenexus query "..."` if your default `~/.cache` lives on
a small partition.

## Pre-seed automation (script-driven, canonical Windows install)

The manual tar workflow above is the foundation; for repeated installs
or Windows clean-install (where the upstream hf-hub 0.5 has a fresh-
download bug at 49% / 567 MB, see PROJECT.md line 98), use the
`scripts/preseed-hf-cache.sh` automation:

```bash
# From a sibling host's HF cache (after rsync / scp / SMB mount)
bash experiments/poc-retrieval/scripts/preseed-hf-cache.sh \
  --source /path/to/working/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B

# From a tarball produced via the manual section above
bash experiments/poc-retrieval/scripts/preseed-hf-cache.sh \
  --source ~/Downloads/qwen3-cache.tar.gz

# Verify-only -- check target has snapshot/<sha>/model.safetensors > 1GB
bash experiments/poc-retrieval/scripts/preseed-hf-cache.sh --verify-only
```

Defaults: model=`Qwen/Qwen3-Embedding-0.6B`, revision=`97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3`,
target=`${HF_HOME:-~/.cache/huggingface}/hub`. Override via
`--model`, `--revision`, `--target`.

The script uses `cp -rL` for directory mode (resolves symlinks so the
target gets independent real files, no dangling links if blob layouts
differ across hosts) and `tar -xzf` / `tar -xf` for tarballs. On
Windows + git-bash it works without symlink-creation privileges -- the
target snapshot dir contains real files instead of symlinks; hf-hub
loads them either way.

After running pre-seed, set `HF_HUB_OFFLINE=1` (next section) to disable
all network roundtrips and force cache-only loads. This is the
canonical Windows install path -- it bypasses the broken fresh-download
without touching upstream.

To prove the pre-seed cycle is deterministic (R1.c reload probe):

```bash
cd experiments/poc-retrieval
bash eval/r1c_probe.sh
# Asserts byte-identical sha256 across pre-seed cycle:
# pre-seed -> sha256 -> delete snapshot -> re-pre-seed -> sha256 -> diff
```

## HF_HUB_OFFLINE mode

Once the cache is seeded (via either path above), set
`HF_HUB_OFFLINE=1` to disable all network roundtrips and force
the embedder to use cache-only:

```bash
export HF_HUB_OFFLINE=1
./bin/codenexus query "any string"
```

This is the recommended mode for air-gapped environments and CI
runners where reproducibility matters more than catching upstream
bumps. (Note: CodeNexus pins `QWEN3_REVISION` to a specific SHA
anyway -- there is no upstream bump to catch -- but `HF_HUB_OFFLINE`
adds a belt-and-suspenders guarantee.)

## Clash-China-down recovery

If you are in mainland China and your route to `huggingface.co`
goes through Clash, the most common failure mode is "Clash daemon
stopped, all traffic falls back to direct, direct cannot reach
huggingface.co". Two recovery options:

1. **Restart Clash, retry:** the simplest path. Verify with
   `curl -s -o /dev/null -w "%{http_code}\n" https://huggingface.co/`
   -> `200` means route works.

2. **Use a HuggingFace mirror:** set the HF endpoint to a mirror
   such as `https://hf-mirror.com/` (community-maintained,
   HuggingFace-API-compatible):

   ```bash
   export HF_ENDPOINT=https://hf-mirror.com
   ./bin/codenexus query "any string"
   ```

   Mirrors carry the same content-addressed blobs as
   `huggingface.co`, so the pinned `QWEN3_REVISION` SHA still
   identifies the same model bytes. If the mirror's main branch
   diverges, the SHA pin protects you -- the load will fail rather
   than serve a different model.

## Sanity check

After any recovery path:

```bash
ls ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/snapshots/
# MUST list exactly: 97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3
```

If the snapshot directory name does not match `QWEN3_REVISION`
in `embedder.rs`, the load will fail at startup -- this is the
§9.8 version-hash gate working as designed.

To verify the pin is actually functional (R1.d probe -- Phase 4
first slice acceptance gate):

```bash
# Delete refs/main if it exists (it's cache-internal, doesn't
# affect the pinned-revision load path):
rm -f ~/.cache/huggingface/hub/models--Qwen--Qwen3-Embedding-0.6B/refs/main
# Force offline mode:
export HF_HUB_OFFLINE=1
# Load should still succeed because Repo::with_revision uses
# the pinned snapshot dir directly:
./bin/codenexus query "any string"   # MUST succeed
```

If this fails with a network error, the pin is decorative and
not protecting you from upstream re-uploads.
