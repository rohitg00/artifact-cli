# Research note: narrow workers over broad wrappers

`artifact-cli` starts from a simple observation: agents waste context when every integration is exposed as a broad wrapper. The better unit is a narrow worker that owns a specific job and registers a few stable functions.

## Narrow worker pattern

```text
source artifact + job definition
  -> function plan
  -> generated worker scaffold
  -> verification report
  -> iii registry
```

## Existing iii workers to compose with

- `iii-state` for manifests and fingerprints
- `iii-queue` for async generation jobs
- `iii-sandbox` for isolated build/test
- `iii-database` for local mirrors and queryable caches
- `iii-cron` for scheduled refresh
- `iii-http` for external invocation
- `iii-observability` for traces and debugging
- `iii-bridge` for cross-engine sharing

## Design rule

Generate the worker the agent needs, not the entire API surface the provider exposes.
