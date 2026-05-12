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

## Live generated worker demos

These transcripts show the important path: Artifact CLI generates the worker, the generated Rust crate compiles, the worker registers with iii, and `iii trigger` returns live data. The exact stories and launch names will change as the public feeds change.

Run the engine in one terminal:

```bash
iii --use-default-config --no-update-check
```

Expected engine signal:

```text
Engine listening on address: 0.0.0.0:49134
```

### Hacker News

Generate and compile the worker:

```bash
cargo run --bin artifact -- generate \
  --payload examples/hackernews.payload.json \
  --out generated/hackernews-worker

cd generated/hackernews-worker
cargo check
```

Expected generation shape:

```json
{
  "outputDir": "generated/hackernews-worker",
  "workerPath": "generated/hackernews-worker/src/main.rs",
  "manifestPath": "generated/hackernews-worker/artifact.manifest.json",
  "plan": {
    "workerName": "hackernews-worker",
    "namespace": "hackernews",
    "functions": [
      { "functionId": "hackernews::top_stories" },
      { "functionId": "hackernews::get_item" },
      { "functionId": "hackernews::search_cached_stories" }
    ]
  }
}
```

Run the generated worker in another terminal:

```bash
cd generated/hackernews-worker
cargo run --quiet
```

Expected worker signal:

```text
hackernews-worker registered functions against ws://localhost:49134
```

Trigger it through iii:

```bash
iii trigger --use-default-config \
  --function-id hackernews::top_stories \
  --payload '{"limit":5}' \
  --timeout-ms 30000
```

Sample live output:

```json
{
  "functionId": "hackernews::top_stories",
  "items": [
    {
      "rank": 1,
      "title": "Googlebook",
      "score": 223,
      "comments": 293,
      "url": "https://googlebook.google/"
    },
    {
      "rank": 2,
      "title": "CERT is releasing six CVEs for serious security vulnerabilities in dnsmasq",
      "score": 68,
      "comments": 9,
      "url": "https://lists.thekelleys.org.uk/pipermail/dnsmasq-discuss/2026q2/018471.html"
    }
  ],
  "ok": true,
  "source": "https://hacker-news.firebaseio.com/v0/topstories.json"
}
```

### Product Hunt

Generate and compile the worker:

```bash
cargo run --bin artifact -- generate \
  --payload examples/producthunt.payload.json \
  --out generated/producthunt-worker

cd generated/producthunt-worker
cargo check
```

Expected generation shape:

```json
{
  "outputDir": "generated/producthunt-worker",
  "workerPath": "generated/producthunt-worker/src/main.rs",
  "manifestPath": "generated/producthunt-worker/artifact.manifest.json",
  "plan": {
    "workerName": "producthunt-worker",
    "namespace": "producthunt",
    "functions": [
      { "functionId": "producthunt::top_launches" },
      { "functionId": "producthunt::launch_details" },
      { "functionId": "producthunt::maker_lookup" },
      { "functionId": "producthunt::topic_search" },
      { "functionId": "producthunt::launch_metrics" }
    ]
  }
}
```

Run the generated worker:

```bash
cd generated/producthunt-worker
cargo run --quiet
```

Expected worker signal:

```text
producthunt-worker registered functions against ws://localhost:49134
```

Trigger top launches:

```bash
iii trigger --use-default-config \
  --function-id producthunt::top_launches \
  --payload '{"limit":3}' \
  --timeout-ms 30000
```

Sample live output:

```json
{
  "functionId": "producthunt::top_launches",
  "items": [
    {
      "rank": 1,
      "id": "1144799",
      "title": "Free AI SEO Auditor",
      "summary": "Audit your site for the AI search era. 100% Open Source",
      "url": "https://www.producthunt.com/products/free-ai-seo-auditor"
    },
    {
      "rank": 2,
      "id": "1122747",
      "title": "ARKAD Wallet",
      "summary": "The budgeting app you'll actually use.",
      "url": "https://www.producthunt.com/products/arkad-wallet"
    }
  ],
  "ok": true,
  "source": "https://www.producthunt.com/feed"
}
```

Trigger exact launch details from one returned id:

```bash
iii trigger --use-default-config \
  --function-id producthunt::launch_details \
  --payload '{"id":"1144799"}' \
  --timeout-ms 30000
```

Sample live output:

```json
{
  "functionId": "producthunt::launch_details",
  "item": {
    "id": "1144799",
    "rank": 1,
    "title": "Free AI SEO Auditor",
    "summary": "Audit your site for the AI search era. 100% Open Source",
    "url": "https://www.producthunt.com/products/free-ai-seo-auditor"
  },
  "items": [
    {
      "id": "1144799",
      "rank": 1,
      "title": "Free AI SEO Auditor",
      "summary": "Audit your site for the AI search era. 100% Open Source",
      "url": "https://www.producthunt.com/products/free-ai-seo-auditor"
    }
  ],
  "ok": true,
  "source": "https://www.producthunt.com/feed"
}
```

### Digg

Generate and compile the worker:

```bash
cargo run --bin artifact -- generate \
  --payload examples/digg.payload.json \
  --out generated/digg-worker

cd generated/digg-worker
cargo check
```

Expected generation shape:

```json
{
  "outputDir": "generated/digg-worker",
  "workerPath": "generated/digg-worker/src/main.rs",
  "manifestPath": "generated/digg-worker/artifact.manifest.json",
  "plan": {
    "workerName": "digg-worker",
    "namespace": "digg",
    "functions": [
      { "functionId": "digg::top_stories" },
      { "functionId": "digg::author_rank" },
      { "functionId": "digg::search_stories" },
      { "functionId": "digg::story_highlights" },
      { "functionId": "digg::pipeline_status" }
    ]
  }
}
```

Run the generated worker:

```bash
cd generated/digg-worker
cargo run --quiet
```

Expected worker signal:

```text
digg-worker registered functions against ws://localhost:49134
```

Trigger top stories:

```bash
iii trigger --use-default-config \
  --function-id digg::top_stories \
  --payload '{"limit":3}' \
  --timeout-ms 30000
```

Sample live output:

```json
{
  "functionId": "digg::top_stories",
  "items": [
    {
      "rank": 1,
      "title": "Thinking Machines introduces Interaction Models for real-time collaboration",
      "summary": "Roon Claims Slate Star Codex Served As Retrocausal Influencer Marketing"
    },
    {
      "rank": 2,
      "title": "Google detects first AI-developed zero-day exploit",
      "summary": "Roon Claims Slate Star Codex Served As Retrocausal Influencer Marketing"
    }
  ],
  "ok": true,
  "source": "https://di.gg/ai"
}
```

Trigger search and status:

```bash
iii trigger --use-default-config \
  --function-id digg::search_stories \
  --payload '{"query":"AI","limit":2}' \
  --timeout-ms 30000

iii trigger --use-default-config \
  --function-id digg::pipeline_status \
  --payload '{}' \
  --timeout-ms 30000
```

Sample status output:

```json
{
  "functionId": "digg::pipeline_status",
  "ok": true,
  "source": "https://di.gg/ai",
  "status": "Posts:"
}
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
