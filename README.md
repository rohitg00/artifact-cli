# artifact-cli

Turn APIs, specs, docs, and workflow artifacts into narrowly scoped iii workers.

`artifact-cli` is a research project for agent-operable backend surfaces: instead of giving an agent a giant API wrapper or asking it to read docs at runtime, generate a focused worker with a small set of precise functions.

```text
artifact -> narrow iii worker -> callable functions
```

## Why this exists

Agents are better when they call stable functions instead of browsing docs, guessing endpoints, or stitching workflows from scratch. `artifact-cli` creates small iii-native workers around a specific job:

- `linear_risk::blocked_issues`
- `github_repo::stale_prs`
- `docs_search::answer_with_sources`
- `hn::top_stories`

The point is not to generate every endpoint. The point is to generate the few functions an agent actually needs.

## How it fits iii

`artifact-cli` composes with existing workers from [workers.iii.dev](https://workers.iii.dev/):

- `iii-state` — store manifests, source fingerprints, generated worker metadata
- `iii-queue` — run generation and verification asynchronously
- `iii-cron` — refresh synced artifacts on a schedule
- `iii-database` — back generated workers with SQLite/Postgres mirrors
- `iii-sandbox` — build and test generated workers in isolation
- `iii-http` — expose generated functions as HTTP endpoints
- `iii-observability` — traces, logs, and generation/debug telemetry
- `iii-bridge` — share generated workers across iii systems

## Worker functions

The MVP worker registers:

- `artifact::inspect` — classify a source artifact and suggest focused worker functions
- `artifact::plan_worker` — produce a narrow worker plan from an artifact description
- `artifact::generate_worker` — generate a TypeScript iii worker scaffold
- `artifact::verify_worker` — run local structural checks on a generated worker
- `artifact::manifest` — create a manifest for registry/publish workflows

## Example

```bash
iii worker add iii-state
iii worker add iii-queue
iii worker add iii-sandbox
iii worker add ./workers/artifact-cli-worker

iii trigger --function-id='artifact::plan_worker' --payload='{
  "name": "hackernews",
  "goal": "give agents focused access to top stories and item lookup",
  "sourceType": "docs",
  "source": "https://github.com/HackerNews/API",
  "functions": ["top_stories", "get_item", "search_cached_stories"]
}'
```

## Generated worker shape

```text
generated/hackernews-worker/
  package.json
  src/worker.ts
  artifact.manifest.json
  README.md
```

Each generated worker registers narrow iii functions, e.g.

```text
hackernews::top_stories
hackernews::get_item
hackernews::search_cached_stories
```

## Principles

1. **Narrow beats generic** — generate workers around jobs, not every endpoint.
2. **Functions over docs** — agents call `function_id` instead of reading docs at runtime.
3. **Composable by default** — use existing iii workers for state, queues, cron, database, HTTP, sandboxing, and observability.
4. **Inspectable artifacts** — every generated worker ships with a manifest and verification report.
5. **No hidden side effects** — generated functions should declare whether they read, write, sync, or call external systems.

## Status

Early MVP scaffold. The first implementation focuses on planning and generating TypeScript iii worker skeletons. Real source ingestion and verification can be added behind the same function IDs.
