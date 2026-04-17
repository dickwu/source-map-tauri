# source-map-tauri

`source-map-tauri` is a Rust CLI that statically scans Tauri applications and emits Meilisearch-ready NDJSON for artifacts, edges, warnings, and project metadata.

It is built for source-code indexing, not runtime inspection. The scanner walks frontend code, Rust commands, plugin permissions, capabilities, sourcemap metadata, and tests without executing the target application.

## What it does

- Scans Tauri repositories into `artifacts.ndjson`, `edges.ndjson`, and `warnings.ndjson`
- Extracts frontend components, hooks, tests, Tauri commands, plugin metadata, permissions, and capability documents
- Extracts frontend HTTP wrappers, transports, endpoints, and canonical endpoint flows such as `LoginModal -> useLogin -> usePostApi -> POST /auth/login`
- Applies redaction, PHI detection, and risk tagging before documents are written
- Emits Meilisearch settings and project metadata so the bundle can be uploaded or reindexed
- Validates bundle structure, document ids, and command-to-permission evidence
- Supports bounded sourcemap trace lookups for bundle paths

## Current status

This is a working v0.1 baseline focused on static, fixture-backed coverage.

What is already in place:

- CLI commands for `init`, `doctor`, `scan`, `upload`, `reindex`, `search`, `trace`, `validate`, and `print-schema`
- Static extraction for frontend files, Rust command surfaces, Tauri configs, capabilities, and plugin permissions
- Security-oriented document shaping with required metadata on every artifact document
- Fixture coverage for a custom plugin app and an inline-command app
- CI for formatting, linting, tests, and the fixture acceptance path
- Tagged release automation with Homebrew formula updates in `dickwu/homebrew-tap`

What is still being hardened:

- More real-world Tauri repo shapes outside the current fixtures
- Richer sourcemap frame tracing beyond the placeholder bundle-path response
- Live Meilisearch integration coverage in CI

## Why this exists

Generic text search does not preserve the relationships a Tauri codebase actually cares about:

- frontend component -> hook
- hook -> invoke call
- component -> API wrapper -> transport -> backend route
- invoke call -> Rust command
- command -> permission
- permission -> effective capability
- artifact -> related tests

`source-map-tauri` exists to build that graph statically and safely. When the code is ambiguous, it prefers emitting a warning document over silently skipping it.

## Constraints

- Static analysis only. Do not execute the target app.
- Fixture-safe by default. No secrets or PHI literals should land in fixtures or emitted documents.
- Every artifact document includes `id`, `repo`, `kind`, `risk_level`, `contains_phi`, `has_related_tests`, and `related_tests`.
- Document ids are Meilisearch-safe and validation will fail if they are not.

## Requirements

Build requirements:

- Rust toolchain
- A Tauri repository to scan
- Meilisearch only when using `upload`, `reindex`, `search`, or Meili health checks in `doctor`

The current CI runs on Linux and macOS.

## Install

Install from crates.io:

```bash
cargo install source-map-tauri
```

Install from Homebrew:

```bash
brew tap dickwu/tap
brew install source-map-tauri
```

Or build from source:

```bash
git clone git@github.com:dickwu/source-map-tauri.git
cd source-map-tauri
cargo build --release
```

The binary will be at:

```bash
./target/release/source-map-tauri
```

## Quick start

### 1. Scaffold config

```bash
source-map-tauri init --root /path/to/tauri-app
```

`init` creates:

- `.repo-search/tauri/source-map-tauri.toml`
- `.repo-search/tauri/.gitignore`
- `~/.config/meilisearch/connect.json` with placeholder values if it does not already exist

### 2. Check repository shape

```bash
source-map-tauri doctor --root /path/to/tauri-app --repo my-tauri-app
```

`doctor` reports whether the repo looks like a Tauri app, how many frontend/capability/permission files were found, and whether Vite sourcemaps appear configurable.

### 3. Scan a repository

```bash
source-map-tauri scan --root /path/to/tauri-app --repo my-tauri-app --out /tmp/source-map-tauri
```

The scan bundle includes:

- `artifacts.ndjson`
- `edges.ndjson`
- `warnings.ndjson`
- `summary.json`
- `project-info.json`
- `meili-settings.json`

### 4. Validate the bundle

```bash
source-map-tauri validate --input /tmp/source-map-tauri
```

### 5. Upload to Meilisearch

```bash
source-map-tauri upload \
  --input /tmp/source-map-tauri/artifacts.ndjson \
  --edges /tmp/source-map-tauri/edges.ndjson \
  --warnings /tmp/source-map-tauri/warnings.ndjson \
  --wait
```

Connection resolution order:

1. `--meili-url` / `--meili-key`
2. `MEILI_HOST`, `MEILI_MASTER_KEY`, `MEILI_SEARCH_KEY`
3. `~/.config/meilisearch/connect.json`
4. The default `http://127.0.0.1:7700` host in config

### 6. Re-scan and upload in one step

```bash
source-map-tauri reindex --root /path/to/tauri-app --repo my-tauri-app --wait
```

### 7. Search indexed documents

```bash
source-map-tauri search --query "patient upload permission"
```

For frontend endpoint queries, use the normalized path:

```bash
source-map-tauri search --query "auth/login"
```

Endpoint-shaped queries are normalized to `/auth/login` and automatically filtered to
`frontend_http_flow`. That means the query returns one canonical flow document per repo
instead of separate hits for the wrapper, transport, and every callsite.

Example flow shape:

```json
{
  "kind": "frontend_http_flow",
  "display_name": "POST /auth/login",
  "normalized_path": "/auth/login",
  "primary_component": "LoginModal",
  "primary_wrapper": "useLogin",
  "primary_transport": "usePostApi",
  "primary_flow": [
    {
      "kind": "frontend_component",
      "name": "LoginModal",
      "path": "src/components/extra/LoginModal.tsx"
    },
    {
      "kind": "frontend_api_wrapper",
      "name": "useLogin",
      "path": "src/utils/apis/auth.ts"
    },
    {
      "kind": "frontend_transport",
      "name": "usePostApi",
      "path": "src/utils/apis/api.ts"
    },
    {
      "kind": "frontend_http_endpoint",
      "method": "POST",
      "path": "/auth/login"
    }
  ]
}
```

### 8. Print JSON schema hints

```bash
source-map-tauri print-schema --kind artifact
source-map-tauri print-schema --kind edge
source-map-tauri print-schema --kind warning
```

### 9. Trace a generated bundle frame

```bash
source-map-tauri trace --root /path/to/tauri-app --bundle dist/app.js --line 120 --column 4
```

`trace` currently verifies the bundle path and preserves the command surface while deeper sourcemap tracing is being filled in.

## CLI reference

```bash
source-map-tauri init [--root <ROOT>] [--repo <REPO>] [--config <CONFIG>]
source-map-tauri doctor [--root <ROOT>] [--repo <REPO>] [--config <CONFIG>]
source-map-tauri scan [--root <ROOT>] [--repo <REPO>] [--out <OUT>] [--include-node-modules] [--include-target] [--include-dist] [--include-vendor]
source-map-tauri upload --input <ARTIFACTS> [--edges <EDGES>] [--warnings <WARNINGS>] [--index <INDEX>] [--meili-url <URL>] [--meili-key <KEY>] [--wait]
source-map-tauri reindex [--root <ROOT>] [--repo <REPO>] [--out <OUT>] [--index <INDEX>] [--meili-url <URL>] [--meili-key <KEY>] [--wait]
source-map-tauri search --query <QUERY> [--index <INDEX>] [--filter <FILTER>] [--limit <LIMIT>] [--meili-url <URL>] [--meili-key <KEY>]
source-map-tauri trace --bundle <BUNDLE> --line <LINE> [--column <COLUMN>] [--root <ROOT>]
source-map-tauri validate --input <DIR>
source-map-tauri print-schema --kind artifact|edge|warning
```

Global flags:

- `--strict`
- `--verbose`
- `--quiet`
- `--redact-secrets`
- `--detect-phi`
- `--fail-on-phi`

## Frontend HTTP flow search

The frontend scanner now materializes one endpoint flow per `repo + method + normalized_path`.

For code shaped like:

```ts
// src/components/extra/LoginModal.tsx
const { refetch: attemptLogin } = useLogin(email, password, false)
await attemptLogin()

// src/utils/apis/auth.ts
export const useLogin = (email: string, password: string, enabled: boolean) =>
  usePostApi('auth/login', { email, password }, false, enabled)

// src/utils/apis/api.ts
export const usePostApi = (path: string, data: unknown, ...) =>
  tauriFetch(`${API_URL}/${path}`, { method: 'POST', body: JSON.stringify(data) })
```

`source-map-tauri` emits:

- `frontend_api_wrapper` for `useLogin`
- `frontend_transport` for `usePostApi`
- `frontend_http_endpoint` for `POST /auth/login`
- one aggregated `frontend_http_flow` for `/auth/login`

If the same route is used from multiple places, the index still emits one
`frontend_http_flow` document. Extra callsites are collapsed into metadata like
`caller_count`, `alternate_components`, and `source_paths`.

## Acceptance path

Primary verification commands:

```bash
cargo test
cargo run -- scan --root tests/fixtures/tauri-custom-plugin --repo fixture --out /tmp/smt-fixture
cargo run -- validate --input /tmp/smt-fixture
```

## Release and Homebrew

Tagged releases build macOS and Linux binaries, create a GitHub release, and update the Homebrew formula in [`dickwu/homebrew-tap`](https://github.com/dickwu/homebrew-tap).

The release workflow expects a repository secret:

- `HOMEBREW_TAP_TOKEN`: a GitHub token with permission to update `dickwu/homebrew-tap`
