//! Cross-cutting graph primitives shared between core and parser sub-crate.
//!
//! Locked vocabulary per `/CONTEXT.md` (commit a0fa4d8). Adding new types
//! here requires a CONTEXT.md update first.

/// File-scoped name aliasing from an import declaration. Persisted in
/// `alias_decls` table after Phase 04.5-03 W0 (schema migration). Replaces
/// the in-memory `namespace_aliases: HashMap<(String, String), String>` in
/// the pre-04.5-03 EdgeBuilder, AND extends coverage to named / renamed /
/// default imports (the pre-04.5-03 monolith only persisted namespace
/// aliases in-memory; named/renamed/default were resolved via separate
/// SQL queries on `edges.kind='Imports'` rows).
///
/// Four import variants per CONTEXT.md lines 47-52:
/// - named:     `import { foo } from "./X"`     -> alias=`foo`, target_member=Some("foo")
/// - renamed:   `import { foo as bar } from "./X"` -> alias=`bar`, target_member=Some("foo")
/// - namespace: `import * as X from "./Y"`      -> alias=`X`, target_member=None
/// - default:   `import X from "./Y"`           -> alias=`X`, target_member=Some("default")
// `dead_code` allowed: consumed by W1+ parser sub-crate (Phase 04.5-03 wave 1
// onwards). API surface lands in W0 so W1 can compile against it without a
// schema-and-API double-deploy.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasDecl {
    pub from_file: String,
    pub alias: String,
    pub target_file: String,
    pub target_member: Option<String>,
}
