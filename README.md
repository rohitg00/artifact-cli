# artifact-cli

Turn APIs, specs, docs, and workflow artifacts into narrowly scoped **Rust iii workers**.

`artifact-cli` is a research project for agent-operable backend surfaces: instead of giving an agent a giant API wrapper or asking it to read docs at runtime, generate a focused Rust worker with a small set of precise iii functions and a concrete reuse plan for the iii engine and `iii-hq/workers`.

```text
artifact -> narrow Rust iii worker -> callable functions
```

## Why this exists

Agents are better when they call stable functions instead of browsing docs, guessing endpoints, or stitching workflows from scratch. `artifact-cli` creates small iii-native Rust workers around a specific job:

- `linear_risk::blocked_issues`
- `github_repo::stale_prs`
- `docs_search::answer_with_sources`
- `hn::top_stories`

The point is not to generate every endpoint. The point is to generate the few functions an agent actually needs.

## Why Rust

`artifact-cli` is infrastructure: parsing, planning, generation, verification, packaging, filesystem work, and eventually worker registry publishing. Rust gives us a single binary, strong manifests, safer execution boundaries, and a cleaner path to binary workers for the iii ecosystem.

## How it fits iii

`artifact-cli` composes with prebuilt iii surfaces instead of rebuilding platform plumbing:

- `iii-hq/iii` builtins — state, queue, cron, REST, stream, sandbox, observability
- `iii-hq/workers` modules — credentials, shell, filesystem, database, MCP, skills, proof, model providers, hooks, sessions, policy

The generated worker should only own the artifact-specific function logic. Storage, async execution, auth, local mirrors, browser verification, MCP exposure, and observability are delegated to reusable workers.

## Current Rust iii primitives

The Rust worker registers the same `artifact::*` function surface through `iii-sdk`:

- `artifact::inspect` — classify a source artifact and suggest focused worker functions
- `artifact::catalog` — list reusable iii engine builtins and installable `iii-hq/workers`
- `artifact::plan_worker` — produce a narrow worker plan from an artifact description
- `artifact::generate_worker` — generate a Rust iii worker scaffold
- `artifact::verify_worker` — run structural checks on a generated worker
- `artifact::manifest` — create a manifest preview for registry/publish workflows

Run it as a live iii worker:

```bash
cargo run --bin artifact-cli-worker -- serve --iii-url ws://localhost:49134
```

The primary CLI is the human doorway. It generates workers, verifies them, prints install commands, and can call registered iii functions:

```bash
cargo run --bin artifact -- catalog
cargo run --bin artifact -- recipes

cargo run --bin artifact -- from https://github.com/HackerNews/API \
  --goal "give agents focused access to top stories and item lookup" \
  --source-type docs
```

Generate a Rust worker scaffold from a JSON payload:

```bash
cargo run --bin artifact -- generate \
  --payload examples/hackernews.payload.json \
  --out ./generated/hackernews-worker
```

Try the Digg AI example:

```bash
cargo run --bin artifact -- from https://di.gg/ai \
  --goal "answer rank lookup, top stories, story highlights, search, and pipeline status"

cargo run --bin artifact -- generate \
  --payload examples/digg.payload.json \
  --out ./generated/digg-worker
```

Recipes are curated, not exhaustive. Each recipe has a stage, priority, integration surface, research links, and a reason it belongs in the roadmap.

Build-now recipes are small read-only surfaces with low setup risk: Digg, Hacker News, arXiv, and Wikipedia.

Research-first recipes have stronger auth, schema, rate-limit, privacy, or write-safety questions: Product Hunt, Linear, GitHub repo risk, Stripe, Sentry, Slack, Notion, and OpenRouter.

Preview the iii manifest:

```bash
cargo run --bin artifact -- manifest --payload examples/hackernews.payload.json
```

Verify it:

```bash
cargo run --bin artifact -- verify ./generated/hackernews-worker
```

Print the dependency/build/run plan:

```bash
cargo run --bin artifact -- install ./generated/hackernews-worker
```

Call a registered generated function:

```bash
cargo run --bin artifact -- call hackernews::top_stories --json '{"limit":10}'
```

## Generated worker shape

```text
generated/hackernews-worker/
  Cargo.toml
  src/main.rs
  iii.worker.yaml
  artifact.manifest.json
  README.md
```

Each generated Rust worker keeps function IDs explicit, e.g.

```text
hackernews::top_stories
hackernews::get_item
hackernews::search_cached_stories
```

Each generated plan also includes:

- `usesWorkers` — all selected iii builtins and installable workers
- `reusePlan.engineBuiltins` — functions already provided by `iii-hq/iii`
- `reusePlan.installableWorkers` — `iii worker add <name>` dependencies from `iii-hq/workers`
- `reusePlan.missingCapabilities` — anything artifact-cli could not map to a reusable worker

## Principles

1. **Rust-first** — core, CLI, worker runtime, and generated workers should be Rust.
2. **Narrow beats generic** — generate workers around jobs, not every endpoint.
3. **Functions over docs** — agents call `function_id` instead of reading docs at runtime.
4. **Composable by default** — use existing iii workers for state, queues, cron, database, HTTP, sandboxing, and observability.
5. **Inspectable artifacts** — every generated worker ships with a manifest and verification report.
6. **No hidden side effects** — generated functions should declare whether they read, write, sync, or call external systems.

## Recipe graduation

A recipe only moves from `research_first` to `build_now` when it has:

- a stable official or public source to integrate with
- one repeated agent job that is clearer than a general API wrapper
- known auth, scope, rate-limit, and cache behavior
- an iii reuse plan for state, credentials, HTTP, database, observability, and MCP exposure
- a smoke test that generates, verifies, and compiles the worker scaffold

## Development

```bash
cargo fmt
cargo test
cargo run --bin artifact -- catalog
cargo run --bin artifact -- from https://github.com/HackerNews/API --goal "top stories"
```

## Status

Production Rust worker plus primary `artifact` CLI. `artifact-cli-worker` can run as a live `iii-sdk` worker and register the full `artifact::*` function surface. Generated workers are also Rust and use `iii-sdk` registration APIs directly.
