# spec-to-worker

Turn OpenAPI specs and MCP servers into normal triggerable iii functions.

`spec-to-worker` exposes one public worker function:

```text
spec-to-worker::convert
```

The conversion path is:

```text
OpenAPI or MCP input -> spec-to-worker::convert -> engine HTTP-invoked iii functions
```

After conversion, agents, users, and other workers call the generated functions
with ordinary `iii trigger`. They do not need to know whether a function came
from OpenAPI, MCP over HTTP, or MCP over stdio.

## Product Contract

Spec-to-worker is not a registry of hand-written integrations. It should work
for any compatible OpenAPI document or MCP server the user provides.

1. The user provides an OpenAPI URL/file, an MCP HTTP URL, or an MCP stdio
   command.
2. Spec-to-worker discovers the callable surface.
3. Spec-to-worker registers one iii function per operation or tool.
4. The engine routes those functions through HTTP invocation.
5. The engine lists the converted source as a normal engine-runtime worker group.

No generated worker process is started for converted sources. The grouping exists
because it is useful to users browsing workers, but the functions are just normal
engine functions.

| Input | Discovery | Registered output |
| --- | --- | --- |
| OpenAPI JSON/YAML | `paths`, operations, methods, params, request/response schemas | One iii function per operation |
| MCP HTTP URL | MCP initialize and `tools/list` | One iii function per MCP tool |
| MCP stdio command | Spawn command, initialize, `tools/list` | One iii function per MCP tool |

## Current Status

Working locally:

- OpenAPI JSON and YAML conversion
- MCP HTTP tool discovery and invocation
- MCP stdio tool discovery and invocation
- Function replacement when the same source is converted again
- Public `engine::functions::list` without private `iii` metadata
- Public `engine::workers::list` with generated engine-runtime worker groups

Still pre-v1:

- Auth handoff for protected OpenAPI endpoints is not done.
- Write-capable APIs need a stricter safety policy before broad use.
- The paired iii engine changes must ship with spec-to-worker for worker grouping
  and trusted local bridge routing.

## Install

Build from this repo:

```bash
cargo build
```

Run the iii engine in one terminal:

```bash
iii --use-default-config --no-update-check
```

Run spec-to-worker in another terminal:

```bash
cargo run --bin spec-to-worker -- serve --iii-url ws://localhost:49134
```

Expected worker signal:

```text
spec-to-worker registered 1 spec-to-worker::* iii functions against ws://localhost:49134
```

## Convert OpenAPI

Example using the public XKCD OpenAPI spec:

```bash
iii trigger \
  --function-id spec-to-worker::convert \
  --payload '{"name":"xkcd live","sourceType":"open_api","url":"https://api.apis.guru/v2/specs/xkcd.com/1.0.0/openapi.json"}' \
  --timeout-ms 120000
```

The response includes the registered iii functions:

```json
{
  "ok": true,
  "mode": "http_invocation",
  "workerName": "xkcd-live-worker",
  "namespace": "xkcd_live",
  "functionCount": 2,
  "registeredFunctions": [
    {
      "functionId": "xkcd_live::get_info_0_json",
      "method": "GET",
      "url": "http://xkcd.com/info.0.json"
    },
    {
      "functionId": "xkcd_live::get_comicid_info_0_json",
      "method": "GET",
      "url": "http://xkcd.com/{comicId}/info.0.json"
    }
  ]
}
```

Call the new function normally:

```bash
iii trigger \
  --function-id xkcd_live::get_comicid_info_0_json \
  --payload '{"comicId":"614"}' \
  --timeout-ms 120000
```

## Convert MCP Stdio

Use any MCP stdio server command:

```bash
iii trigger \
  --function-id spec-to-worker::convert \
  --payload '{"name":"docs mcp","sourceType":"mcp","command":"npx","args":["-y","some-mcp-server"]}' \
  --timeout-ms 180000
```

Spec-to-worker starts the command, calls `tools/list`, and registers each tool:

```json
{
  "ok": true,
  "mode": "http_invocation",
  "workerName": "docs-mcp-worker",
  "namespace": "docs_mcp",
  "functionCount": 2,
  "registeredFunctions": [
    {
      "functionId": "docs_mcp::search_docs",
      "method": "POST",
      "url": "stdio:npx -y some-mcp-server"
    },
    {
      "functionId": "docs_mcp::read_doc",
      "method": "POST",
      "url": "stdio:npx -y some-mcp-server"
    }
  ]
}
```

Call a registered MCP tool through iii:

```bash
iii trigger \
  --function-id docs_mcp::search_docs \
  --payload '{"query":"React hooks cleanup"}' \
  --timeout-ms 180000
```

## Convert MCP HTTP

Start any MCP HTTP server, then convert its endpoint:

```bash
iii trigger \
  --function-id spec-to-worker::convert \
  --payload '{"name":"docs http","sourceType":"mcp","url":"http://127.0.0.1:18092/mcp"}' \
  --timeout-ms 180000
```

Call the registered function:

```bash
iii trigger \
  --function-id docs_http::search_docs \
  --payload '{"query":"React useMemo example"}' \
  --timeout-ms 180000
```

## Verify Visibility

Converted functions should look like normal functions. Public function metadata
keeps the useful `spec` details and strips private engine fields:

```bash
iii trigger \
  --function-id engine::functions::list \
  --payload '{"include_internal":false}' |
  jq '[.functions[] | select(.function_id == "docs_mcp::search_docs") | {function_id, metadata}]'
```

Expected shape:

```json
[
  {
    "function_id": "docs_mcp::search_docs",
    "metadata": {
      "spec": {
        "mcpTool": "search-docs",
        "mode": "http_invocation",
        "namespace": "docs_mcp",
        "source": "stdio:npx -y some-mcp-server",
        "sourceType": "mcp",
        "workerName": "docs-mcp-worker"
      }
    }
  }
]
```

The generated group should appear as a normal public worker entry:

```bash
iii trigger \
  --function-id engine::workers::list \
  --payload '{}' |
  jq '[.workers[] | select(.name == "docs-mcp-worker")]'
```

Expected shape:

```json
[
  {
    "name": "docs-mcp-worker",
    "runtime": "engine",
    "functions": ["docs_mcp::search_docs", "docs_mcp::read_doc"]
  }
]
```

The exact worker fields depend on the engine version, but the entry should not
expose a generated process, isolation runtime, or private grouping metadata.

## Public Payloads

`spec-to-worker::convert` accepts:

| Field | Required | Notes |
| --- | --- | --- |
| `name` | Usually | Human name used to create the namespace and worker group. |
| `sourceType` | Recommended | Public conversion path is `open_api` or `mcp`. Obvious MCP URLs, MCP commands, and OpenAPI-looking filenames can be inferred. |
| `url` / `source` | For OpenAPI and MCP HTTP | URL or local path for the input. `url` is accepted as an alias for `source`. |
| `command` | For MCP stdio | Program to start, such as `npx`, `node`, or `python`. |
| `args` | Optional | Command arguments for MCP stdio. |
| `env` | Optional | Environment variables for MCP stdio. |
| `functions` | Optional | MCP tool-name filter for advanced use. Omit it for full automatic tool discovery. |
| `goal` | Optional | Short intent label. |

The inline JSON shown in the examples is only the iii trigger payload. It is not
an integration file.

## Function IDs

Spec-to-worker normalizes names into iii-safe IDs:

| Input | Function ID |
| --- | --- |
| `xkcd live` + `get /{comicId}/info.0.json` | `xkcd_live::get_comicid_info_0_json` |
| `docs mcp` + `search-docs` | `docs_mcp::search_docs` |
| `github mcp` + `search_repos` | `github_mcp::search_repos` |

If a duplicate ID appears during conversion, spec-to-worker appends a numeric
suffix. If the same source is converted again, the old generated function ref is
unregistered and replaced.

## Developer Commands

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

No release has been created from these changes yet.
