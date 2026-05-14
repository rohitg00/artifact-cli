use spec_to_worker::{
    convert_spec_to_worker_for_iii, registered_function_ids, worker_metadata,
    ConvertSpecToWorkerInput, SourceType, SpecToWorkerError,
};

#[test]
fn exposes_only_convert_as_public_worker_surface() {
    assert_eq!(registered_function_ids(), vec!["spec-to-worker::convert"]);

    let metadata = worker_metadata();
    assert_eq!(metadata.runtime, "rust");
    assert_eq!(metadata.name, "spec-to-worker");
}

#[test]
fn convert_rejects_blank_source_before_any_registration() {
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let error = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("blank".into()),
            source: Some(" \n\t ".into()),
            ..Default::default()
        },
    )
    .unwrap_err();
    iii.shutdown();

    assert!(matches!(error, SpecToWorkerError::InvalidInput(_)));
    assert!(error.to_string().contains("source/url cannot be blank"));
}

#[test]
fn convert_rejects_non_openapi_non_mcp_sources() {
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let error = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("plain url".into()),
            source_type: Some(SourceType::Url),
            source: Some("https://example.com".into()),
            ..Default::default()
        },
    )
    .unwrap_err();
    iii.shutdown();

    assert!(matches!(error, SpecToWorkerError::InvalidInput(_)));
    assert!(error
        .to_string()
        .contains("supports open_api and mcp sources"));
}

#[test]
fn convert_registers_openapi_json_as_http_invoked_functions() {
    let tmp = tempfile::tempdir().unwrap();
    let spec_path = tmp.path().join("demo.openapi.json");
    std::fs::write(
        &spec_path,
        r#"{
  "openapi": "3.0.0",
  "info": { "title": "Demo API", "version": "1.0.0" },
  "servers": [{ "url": "https://api.example.com/v1" }],
  "paths": {
    "/search": {
      "get": {
        "operationId": "search_items",
        "summary": "Search items",
        "parameters": [
          { "name": "q", "in": "query", "required": true, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": {
            "description": "ok",
            "content": { "application/json": { "schema": { "type": "object" } } }
          }
        }
      }
    },
    "/items/{id}": {
      "get": {
        "operationId": "read_item",
        "summary": "Read item",
        "parameters": [
          { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": { "200": { "description": "ok" } }
      }
    }
  }
}"#,
    )
    .unwrap();
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let converted = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("demo".into()),
            source: Some(format!("  {}\n", spec_path.display())),
            ..Default::default()
        },
    )
    .unwrap();
    iii.shutdown();

    assert_eq!(converted.mode, "http_invocation");
    assert_eq!(converted.worker_name, "demo-worker");
    assert_eq!(converted.manifest.worker_name, "demo-worker");
    assert_eq!(converted.manifest.namespace, "demo");
    assert_eq!(converted.namespace, "demo");
    assert_eq!(converted.source_type, SourceType::OpenApi);
    assert_eq!(
        converted.source.as_deref(),
        Some(spec_path.to_str().unwrap())
    );
    assert_eq!(converted.function_count, 2);
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "demo::search_items"
            && function.method == "GET"
            && function.url == "https://api.example.com/v1/search"));
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "demo::read_item"
            && function.method == "GET"
            && function.url == "https://api.example.com/v1/items/{id}"));
    assert!(converted
        .notes
        .iter()
        .any(|note| note.contains("normal iii functions backed by engine HTTP invocation")));
    assert!(converted
        .notes
        .iter()
        .any(|note| note.contains("engine-runtime worker group; no worker process is started")));
}

#[test]
fn convert_accepts_openapi_yaml_specs() {
    let tmp = tempfile::tempdir().unwrap();
    let spec_path = tmp.path().join("demo.openapi.yaml");
    std::fs::write(
        &spec_path,
        r#"openapi: 3.0.0
info:
  title: Demo API
  version: 1.0.0
servers:
  - url: https://api.example.com
paths:
  /status:
    get:
      operationId: read_status
      summary: Read status
      responses:
        "200":
          description: ok
"#,
    )
    .unwrap();
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let converted = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("demo".into()),
            source: Some(spec_path.display().to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    iii.shutdown();

    assert_eq!(converted.mode, "http_invocation");
    assert_eq!(converted.source_type, SourceType::OpenApi);
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "demo::read_status"
            && function.method == "GET"
            && function.url == "https://api.example.com/status"));
}

#[test]
fn convert_replaces_existing_http_invoked_functions() {
    let tmp = tempfile::tempdir().unwrap();
    let spec_path = tmp.path().join("demo.openapi.json");
    std::fs::write(
        &spec_path,
        r#"{
  "openapi": "3.0.0",
  "info": { "title": "Demo API", "version": "1.0.0" },
  "servers": [{ "url": "https://api.example.com" }],
  "paths": {
    "/status": {
      "get": {
        "operationId": "read_status",
        "summary": "Read status",
        "responses": { "200": { "description": "ok" } }
      }
    }
  }
}"#,
    )
    .unwrap();
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());
    let input = ConvertSpecToWorkerInput {
        name: Some("replace demo".into()),
        source: Some(spec_path.display().to_string()),
        ..Default::default()
    };

    let first = convert_spec_to_worker_for_iii(&iii, input.clone()).unwrap();
    let second = convert_spec_to_worker_for_iii(&iii, input).unwrap();
    iii.shutdown();

    assert_eq!(first.mode, "http_invocation");
    assert_eq!(second.mode, "http_invocation");
    assert_eq!(second.function_count, 1);
    assert_eq!(
        second.registered_functions[0].function_id,
        "replace_demo::read_status"
    );
}

#[test]
fn convert_registers_named_mcp_endpoint_functions() {
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let converted = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("github mcp".into()),
            source_type: Some(SourceType::Mcp),
            source: Some("https://example.com/mcp".into()),
            functions: vec!["search_repos".into(), "get_issue".into()],
            ..Default::default()
        },
    )
    .unwrap();
    iii.shutdown();

    assert_eq!(converted.mode, "http_invocation");
    assert_eq!(converted.namespace, "github_mcp");
    assert_eq!(converted.function_count, 2);
    assert!(converted
        .registered_functions
        .iter()
        .all(|function| function.method == "POST" && function.url == "https://example.com/mcp"));
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "github_mcp::search_repos"));
}

#[test]
fn conversion_response_describes_normal_engine_grouping_without_a_process() {
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let converted = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("docs mcp".into()),
            source_type: Some(SourceType::Mcp),
            source: Some("https://example.com/mcp".into()),
            functions: vec!["search-docs".into()],
            ..Default::default()
        },
    )
    .unwrap();
    iii.shutdown();

    assert_eq!(converted.mode, "http_invocation");
    assert_eq!(converted.worker_name, "docs-mcp-worker");
    assert_eq!(converted.namespace, "docs_mcp");
    assert_eq!(converted.manifest.worker_name, "docs-mcp-worker");
    assert_eq!(
        converted.manifest.functions[0].function_id,
        "docs_mcp::search_docs"
    );
    assert!(converted
        .notes
        .iter()
        .any(|note| note.contains("normal iii functions backed by engine HTTP invocation")));
    assert!(converted
        .notes
        .iter()
        .any(|note| note.contains("engine-runtime worker group; no worker process is started")));
}

#[test]
fn convert_discovers_mcp_http_tools() {
    let url = spawn_mock_mcp_server(true);
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let converted = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("docs mcp".into()),
            source: Some(url.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    iii.shutdown();

    assert_eq!(converted.mode, "http_invocation");
    assert_eq!(converted.source_type, SourceType::Mcp);
    assert_eq!(converted.namespace, "docs_mcp");
    assert_eq!(converted.function_count, 2);
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "docs_mcp::search_docs"
            && function.method == "POST"
            && function.url == url));
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "docs_mcp::read_doc"));
}

#[test]
fn convert_discovers_mcp_stdio_tools() {
    let tmp = tempfile::tempdir().unwrap();
    let script_path = tmp.path().join("mcp-stdio.sh");
    std::fs::write(
        &script_path,
        r#"read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"mock-stdio","version":"1.0.0"}}}'
read -r line
read -r line
case "$line" in
  *tools/list*)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"search-docs","description":"Search docs over stdio","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}},{"name":"read_doc","description":"Read doc over stdio","inputSchema":{"type":"object","properties":{"id":{"type":"string"}}}}]}}'
    ;;
  *tools/call*)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"stdio ok"}]}}'
    ;;
  *)
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"unknown method"}}'
    ;;
esac
"#,
    )
    .unwrap();
    let iii = iii_sdk::register_worker("ws://localhost:1", spec_to_worker::init_options());

    let converted = convert_spec_to_worker_for_iii(
        &iii,
        ConvertSpecToWorkerInput {
            name: Some("stdio docs".into()),
            command: Some("sh".into()),
            args: vec![script_path.display().to_string()],
            ..Default::default()
        },
    )
    .unwrap();
    iii.shutdown();

    assert_eq!(converted.mode, "http_invocation");
    assert_eq!(converted.source_type, SourceType::Mcp);
    assert_eq!(converted.namespace, "stdio_docs");
    assert_eq!(converted.function_count, 2);
    assert!(converted
        .source
        .as_deref()
        .is_some_and(|source| source.starts_with("stdio:sh ")));
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "stdio_docs::search_docs"
            && function.method == "POST"
            && function.url.starts_with("stdio:sh ")));
    assert!(converted
        .registered_functions
        .iter()
        .any(|function| function.function_id == "stdio_docs::read_doc"));
}

fn spawn_mock_mcp_server(use_sse_for_tools: bool) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(8).flatten() {
            handle_mock_mcp_request(stream, use_sse_for_tools);
        }
    });
    format!("http://{address}/mcp")
}

fn handle_mock_mcp_request(mut stream: std::net::TcpStream, use_sse_for_tools: bool) {
    let body = read_http_body(&mut stream);
    let value = serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default();
    let method = value.get("method").and_then(serde_json::Value::as_str);
    let id = value.get("id").cloned().unwrap_or(serde_json::json!(1));
    match method {
        Some("initialize") => write_http_json(
            &mut stream,
            200,
            Some("mock-session"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "mock", "version": "1.0.0" }
                }
            }),
        ),
        Some("notifications/initialized") => write_http_empty(&mut stream, 202),
        Some("tools/list") => {
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "search-docs",
                            "description": "Search docs",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "query": { "type": "string" } },
                                "required": ["query"]
                            },
                            "outputSchema": {
                                "type": "object",
                                "properties": { "matches": { "type": "array" } }
                            }
                        },
                        {
                            "name": "read_doc",
                            "description": "Read one doc",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "id": { "type": "string" } }
                            }
                        }
                    ]
                }
            });
            if use_sse_for_tools {
                write_http_sse(&mut stream, response);
            } else {
                write_http_json(&mut stream, 200, None, response);
            }
        }
        Some("tools/call") => write_http_json(
            &mut stream,
            200,
            None,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": "ok" }]
                }
            }),
        ),
        _ => write_http_json(
            &mut stream,
            200,
            None,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "unknown method" }
            }),
        ),
    }
}

fn read_http_body(stream: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut buffer = vec![0; 8192];
    let mut received = Vec::new();
    let mut content_length = None;
    loop {
        let read = stream.read(&mut buffer).unwrap_or(0);
        if read == 0 {
            break;
        }
        received.extend_from_slice(&buffer[..read]);
        let request = String::from_utf8_lossy(&received);
        if let Some(header_end) = request.find("\r\n\r\n") {
            if content_length.is_none() {
                content_length = request[..header_end]
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .and_then(|value| value.trim().parse::<usize>().ok());
            }
            let body_start = header_end + 4;
            let expected = content_length.unwrap_or(0);
            if received.len().saturating_sub(body_start) >= expected {
                return String::from_utf8_lossy(&received[body_start..body_start + expected])
                    .to_string();
            }
        }
    }
    String::new()
}

fn write_http_json(
    stream: &mut std::net::TcpStream,
    status: u16,
    session_id: Option<&str>,
    body: serde_json::Value,
) {
    write_http_response(
        stream,
        status,
        "application/json",
        session_id,
        &body.to_string(),
    );
}

fn write_http_sse(stream: &mut std::net::TcpStream, body: serde_json::Value) {
    write_http_response(
        stream,
        200,
        "text/event-stream",
        None,
        &format!("event: message\ndata: {}\n\n", body),
    );
}

fn write_http_empty(stream: &mut std::net::TcpStream, status: u16) {
    write_http_response(stream, status, "text/plain", None, "");
}

fn write_http_response(
    stream: &mut std::net::TcpStream,
    status: u16,
    content_type: &str,
    session_id: Option<&str>,
    body: &str,
) {
    use std::io::Write;
    let status_text = if status == 202 { "Accepted" } else { "OK" };
    let session_header = session_id
        .map(|id| format!("Mcp-Session-Id: {id}\r\n"))
        .unwrap_or_default();
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\n{session_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).unwrap();
}
