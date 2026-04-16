# source-map-tauri Design

This project implements a Rust CLI that statically scans Tauri applications and emits Meilisearch-ready NDJSON describing frontend, Rust, config, plugin, capability, permission, test, and warning artifacts plus cross-linked edge documents.

The approved v0.1 scope is the fixture-backed static pipeline from repo discovery through scan, validation, and upload/search plumbing. The implementation favors safe static parsing, hospital-oriented redaction/risk tagging, and denormalized Meilisearch documents over runtime execution or graph-database semantics.

Primary priorities:

1. Provide a repeatable `init`, `doctor`, `scan`, `validate`, `upload`, `reindex`, `search`, `trace`, and `print-schema` command surface.
2. Emit searchable artifact, edge, and warning documents with stable IDs and risk/test metadata.
3. Cover the custom plugin fixture flow end to end:
   frontend component -> hook use/definition -> plugin guest binding -> plugin command -> permission -> effective capability.
4. Prefer warnings over silent drops when resolution is ambiguous.

Deliberate v0.1 constraints:

- Static analysis only.
- No runtime instrumentation or app execution.
- No PHI or secret retention in generated documents.
- Meilisearch integration should never log keys.
- Trace may start as a bounded implementation as long as the command surface is stable.
