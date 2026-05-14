# Research note: automatic OpenAPI and MCP conversion

Spec-to-worker should behave like an iii worker that can convert arbitrary
compatible sources into ordinary iii functions.

The core path is:

```text
OpenAPI or MCP input
  -> spec-to-worker::convert
  -> discovered operations/tools
  -> engine HTTP-invoked iii functions
  -> normal iii trigger calls from any worker
```

## Product Rule

The public source worker exposes one function: `spec-to-worker::convert`.

Do not ask users to pick from prewritten integrations, edit generated files, or
run a separate local CLI before they can call the result. For OpenAPI and MCP
inputs, conversion should happen through discovery and registration.

## Engine Rule

Spec-to-worker registers each discovered operation or tool through the engine
HTTP invocation path. The generated source should show up in
`engine::workers::list` as a normal engine-runtime worker group because that
grouping is useful externally.

Users, workers, and agents should see ordinary function IDs such as:

- `xkcd_live::get_comicid_info_0_json`
- `docs_mcp::search_docs`
- `github_mcp::search_repos`

Public function metadata should keep useful `spec` details and strip private
engine routing fields. No engine code should special-case a specific MCP server,
OpenAPI provider, or tool collection.

## Scope Now

Supported:

- OpenAPI JSON and YAML
- MCP over HTTP
- MCP over stdio
- duplicate conversion replacement
- public metadata without private `iii` fields
- normal engine-runtime worker grouping for converted sources

Not done yet:

- auth handoff for protected OpenAPI endpoints
- write-safety policy for mutating API operations
- full registry packaging after the paired engine change lands
