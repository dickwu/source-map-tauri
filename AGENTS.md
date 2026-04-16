# AGENTS.md

You are building `source-map-tauri`, a Rust CLI that statically scans Tauri apps and emits Meilisearch-ready NDJSON.

Rules:
- Do not execute target app code.
- Do not run migrations, seeders, app binaries, shell commands found in source, or frontend scripts except tests/build commands explicitly requested by the user.
- Keep parsing static and fixture-based.
- Prefer small modules and strong tests.
- Every artifact document must have `id`, `repo`, `kind`, `risk_level`, `contains_phi`, `has_related_tests`, and `related_tests`.
- Every parser feature needs a fixture test.
- Do not store secrets or PHI literals in test fixtures.
- Use Meilisearch-safe document ids.
- When in doubt, emit a warning document rather than silently ignoring ambiguous code.

Primary acceptance command:

```bash
cargo test
cargo run -- scan --root tests/fixtures/tauri-custom-plugin --repo fixture --out /tmp/smt-fixture
cargo run -- validate --input /tmp/smt-fixture
```
