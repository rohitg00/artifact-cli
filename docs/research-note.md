# Research note: automatic OpenAPI and MCP conversion

Artifact should behave like an iii worker, not a local code generator.

The core path is:

```text
OpenAPI or MCP input
  -> artifact::convert
  -> discovered operations/tools
  -> engine HTTP-invoked iii functions
  -> normal iii trigger calls from any worker
```

## Product rule

The public surface is one function: `artifact::convert`.

Do not ask users to pick from prewritten integrations, edit generated files, or run a separate local CLI before they can call the result. For OpenAPI and MCP inputs, conversion should happen by discovery and registration.

## Engine rule

Artifact registers each discovered operation/tool through the engine HTTP invocation path. The internal grouping hint lives under `metadata.iii.virtualWorker`; the engine consumes and strips that hint before public function listing.

Users, workers, and agents should only see ordinary function ids such as:

- `xkcd_live::get_comicid_info_0_json`
- `context7_stdio::query_docs`
- `docs_mcp::search_docs`

They should not need to know that those functions came from an internal grouped registration.

## Scope now

Supported:

- OpenAPI JSON and YAML
- MCP over HTTP
- MCP over stdio
- duplicate conversion replacement
- public metadata without internal `iii` fields
- hidden internal grouping in `engine::workers::list`

Not done yet:

- auth handoff for protected OpenAPI endpoints
- write-safety policy for mutating API operations
- full registry packaging after the paired engine change lands
