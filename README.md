# artifact-worker

Turn an OpenAPI spec or MCP server into normal iii functions.

`artifact-worker` is the repo and binary name. The user-facing product is the
`artifact::*` worker surface. Its main function is `artifact::convert`.

The goal is simple:

```text
OpenAPI or MCP input -> artifact::convert -> triggerable iii functions
```

After conversion, every other worker calls the result with ordinary
`iii trigger`. Callers do not need to know whether the function came from an
OpenAPI operation, an MCP HTTP server, or an MCP stdio command.

## Why

Large MCP tool lists and full API surfaces waste context. Agents usually need a
small set of stable calls:

- `context7_stdio::query_docs`
- `context7_http::query_docs`
- `xkcd_live::get_comicid_info_0_json`
- `petstore::find_pets_by_status`

Artifact makes those functions available through iii instead of asking the agent
to inspect docs, pick tools from a huge list, or hand-wire API calls each time.

## Automatic Conversion Contract

Artifact is not a prewritten integration collection. The normal path is:

1. The user gives Artifact an OpenAPI URL, an MCP HTTP URL, or an MCP stdio
   command.
2. Artifact discovers the callable surface.
3. Artifact registers the discovered surface as HTTP-invoked iii functions.
4. Any worker triggers those functions normally.

No generated code is required for OpenAPI or MCP conversion.

| Input | What Artifact discovers | What gets registered |
| --- | --- | --- |
| OpenAPI JSON/YAML | `paths`, operations, methods, params, request body schemas, response schemas | One iii function per operation |
| MCP HTTP URL | MCP initialize plus `tools/list` | One iii function per MCP tool |
| MCP stdio command | Spawn command, initialize, `tools/list` | One iii function per MCP tool |

The function IDs are generated from the source name and discovered operation or
tool names. The caller supplies the source; Artifact handles discovery and
registration.

## Current Status

Working locally:

- OpenAPI JSON and YAML conversion
- MCP HTTP tool discovery and invocation
- MCP stdio tool discovery and invocation
- Function replacement when the same artifact is converted again
- Hidden internal grouping inside the engine
- Public `engine::functions::list` without internal metadata
- Public `engine::workers::list` without generated worker entries

Still pre-v1:

- Auth handoff for protected OpenAPI endpoints is not done.
- Write-capable APIs need a stricter safety policy before this should be used
  broadly.
- The paired iii engine changes must ship with artifact-worker for the full
  hidden-group behavior.

## Install

From this repo:

```bash
cargo build
```

Run the iii engine in one terminal:

```bash
iii --use-default-config --no-update-check
```

Run artifact-worker in another terminal:

```bash
cargo run --bin artifact-worker -- serve --iii-url ws://localhost:49134
```

Expected worker signal:

```text
artifact-worker registered 1 artifact::* iii functions against ws://localhost:49134
```

## Convert OpenAPI

Example using the public XKCD OpenAPI spec:

```bash
iii trigger \
  --function-id artifact::convert \
  --payload '{"name":"xkcd live","sourceType":"open_api","url":"https://api.apis.guru/v2/specs/xkcd.com/1.0.0/openapi.json"}' \
  --timeout-ms 120000
```

The response includes the registered iii functions:

```json
{
  "ok": true,
  "mode": "http_invocation",
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

Real output from the current branch:

```json
{
  "num": 614,
  "title": "Woodpecker",
  "safe_title": "Woodpecker",
  "img": "https://imgs.xkcd.com/comics/woodpecker.png"
}
```

## Convert MCP Stdio

Example using the Context7 MCP server through `npx`:

```bash
iii trigger \
  --function-id artifact::convert \
  --payload '{"name":"context7 stdio","sourceType":"mcp","command":"npx","args":["-y","@upstash/context7-mcp"]}' \
  --timeout-ms 180000
```

Artifact starts the command, calls `tools/list`, and registers each tool:

```json
{
  "ok": true,
  "mode": "http_invocation",
  "namespace": "context7_stdio",
  "functionCount": 2,
  "registeredFunctions": [
    {
      "functionId": "context7_stdio::resolve_library_id",
      "method": "POST",
      "url": "stdio:npx -y @upstash/context7-mcp"
    },
    {
      "functionId": "context7_stdio::query_docs",
      "method": "POST",
      "url": "stdio:npx -y @upstash/context7-mcp"
    }
  ]
}
```

Resolve React:

```bash
iii trigger \
  --function-id context7_stdio::resolve_library_id \
  --payload '{"libraryName":"React","query":"React hooks useEffect documentation"}' \
  --timeout-ms 180000
```

Then query docs:

```bash
iii trigger \
  --function-id context7_stdio::query_docs \
  --payload '{"libraryId":"/reactjs/react.dev","query":"React useEffect cleanup example"}' \
  --timeout-ms 180000
```

Real output includes React docs snippets such as:

```text
React useEffect with setTimeout and Cleanup Function
Subscribing to events with React Effect cleanup
Fetching Data with React useEffect and Cleanup
```

## Convert MCP HTTP

Run an MCP HTTP server:

```bash
npx -y @upstash/context7-mcp --transport http --port 18092
```

Convert it:

```bash
iii trigger \
  --function-id artifact::convert \
  --payload '{"name":"context7 http","sourceType":"mcp","url":"http://127.0.0.1:18092/mcp"}' \
  --timeout-ms 180000
```

Call the registered function:

```bash
iii trigger \
  --function-id context7_http::query_docs \
  --payload '{"libraryId":"/reactjs/react.dev","query":"React useMemo example"}' \
  --timeout-ms 180000
```

Real output includes React `useMemo` snippets from Context7.

## Verify Visibility

Converted functions should look like normal functions:

```bash
iii trigger \
  --function-id engine::functions::list \
  --payload '{"include_internal":false}' |
  jq '[.functions[] | select(.function_id == "context7_stdio::query_docs") | {function_id, metadata}]'
```

Expected shape:

```json
[
  {
    "function_id": "context7_stdio::query_docs",
    "metadata": {
      "artifact": {
        "mcpTool": "query-docs",
        "mode": "http_invocation",
        "namespace": "context7_stdio",
        "source": "stdio:npx -y @upstash/context7-mcp",
        "sourceType": "mcp",
        "workerName": "context7-stdio-worker"
      }
    }
  }
]
```

Generated groups should not appear as public workers:

```bash
iii trigger \
  --function-id engine::workers::list \
  --payload '{}' |
  jq '[.workers[] | select(.name == "context7-stdio-worker")]'
```

Expected output:

```json
[]
```

## Public Payloads

`artifact::convert` accepts:

| Field | Required | Notes |
| --- | --- | --- |
| `name` | Usually | Human name used to create the function namespace. |
| `sourceType` | Recommended | Public conversion path is `open_api` or `mcp`. Obvious MCP URLs, MCP commands, and OpenAPI-looking filenames can be inferred. |
| `url` / `source` | For OpenAPI and MCP HTTP | URL or local path for the input. `url` is accepted as an alias for `source`. |
| `command` | For MCP stdio | Program to start, such as `npx`. |
| `args` | Optional | Command arguments for MCP stdio. |
| `env` | Optional | Environment variables for MCP stdio. |
| `functions` | Optional | MCP tool-name filter for advanced use. Omit it for full automatic tool discovery. |
| `goal` | Optional | Short intent label. |

The inline JSON shown in the examples is only the iii trigger payload. It is not
an integration file.

## Function IDs

Artifact normalizes names into iii-safe IDs:

| Input | Function ID |
| --- | --- |
| `xkcd live` + `get /{comicId}/info.0.json` | `xkcd_live::get_comicid_info_0_json` |
| `context7 stdio` + `query-docs` | `context7_stdio::query_docs` |
| `docs mcp` + `search-docs` | `docs_mcp::search_docs` |

If a duplicate ID appears during conversion, Artifact appends a numeric suffix.
If the same artifact is converted again, the old generated function ref is
unregistered and replaced.

## Developer Commands

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Real Checks Run On This Branch

The current local branch was verified with:

- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- iii engine hidden-group tests
- `cargo check -p iii --tests`
- live XKCD OpenAPI conversion and comic lookup
- live Context7 MCP stdio conversion and React docs query
- live Context7 MCP HTTP conversion and React docs query
- duplicate conversion replacement for OpenAPI and MCP stdio
- public function visibility check
- public worker visibility check

No release has been created from these changes yet.
