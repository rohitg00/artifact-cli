# Research note: narrow Rust workers over broad wrappers

`artifact-cli` starts from a simple observation: agents waste context when every integration is exposed as a broad wrapper. The better unit is a narrow Rust worker that owns a specific job, registers a few stable iii functions, and reuses the iii platform for everything that is already solved.

## Narrow worker pattern

```text
source artifact + job definition
  -> function plan
  -> reuse plan
  -> generated Rust worker scaffold
  -> verification report
  -> iii registry
```

## Existing iii surfaces to compose with

Engine builtins from `iii-hq/iii`:

- `iii-state` for manifests, fingerprints, generated worker state, and caches
- `iii-queue` for async generation, verification, sync, and publish jobs
- `iii-cron` for scheduled refresh
- `iii-rest` for HTTP triggers
- `iii-stream` for generation progress and runtime events
- `iii-sandbox` for isolated build, test, and execution
- `iii-observability` for traces, logs, rollups, and alerts

Installable workers from `iii-hq/workers`:

- `auth-credentials` for API keys and OAuth tokens
- `shell-bash` for sandboxed CLI, git, build, and smoke-test commands
- `shell-filesystem` for artifact ingestion and generated file operations
- `iii-database` for SQLite/Postgres/MySQL mirrors and query polling
- `mcp` and `skills` for agent-facing tool/resource exposure
- `proof` for browser verification
- model/provider/session/hook/policy workers when the generated worker needs assistant routing or guardrails

## Design rule

Generate the Rust worker the agent needs, not the entire API surface the provider exposes.

## Reuse rule

If a capability is already in `iii-hq/iii` or `iii-hq/workers`, artifact-cli should record it in the plan and generated manifest instead of generating duplicate code.
