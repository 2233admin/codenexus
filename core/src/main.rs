// codenexus-core: A2A-native code + knowledge graph engine.
//
// Phase -1 / 0 will fill:
//   - axum HTTP server bound to localhost:9876 (configurable, with ~/.codenexus/port lockfile)
//   - A2A protocol endpoints: POST /tasks/send, GET /tasks/{id}, optional SSE stream
//   - tree-sitter parsing pipeline (TS first, multi-lang Phase 2)
//   - candle embedder (Snowflake/BERT family, model weights either embedded or HuggingFace cached)
//   - storage: redb OR rusqlite + sqlite-vec (Phase 0 spike picks)
//   - gix git overlay (blame, log, diff)
//   - graph builder (CALLS edges MVP; IMPORTS/EXTENDS Phase 2)

fn main() {
    println!("codenexus-core: pre-MVP placeholder. See .planning/ for status.");
}
