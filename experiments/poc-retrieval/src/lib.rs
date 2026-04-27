// Library facade so src/bin/*.rs binaries can import internal modules.
//
// Phase 03.6 motivation: enables compute_version_hash.rs to import
// QUERY_INSTRUCT from the embedder module without copy-paste, keeping a
// single source of truth for the §9.8 version-hash compute path.
//
// Only `embedder` is exported here -- main.rs binary still uses `mod embedder;`
// (file-scope re-declaration) to avoid forcing a refactor of the rest of the
// crate's module wiring. This dual setup is intentional: the binary side keeps
// its existing module graph, the library side exposes only what `src/bin/*`
// needs.

pub mod embedder;
