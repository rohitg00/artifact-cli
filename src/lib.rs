use iii_sdk::{
    FunctionRef, HttpInvocationConfig, HttpMethod, InitOptions, RegisterFunction,
    RegisterFunctionMessage, RegisterServiceMessage, WorkerMetadata,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

pub type Result<T> = std::result::Result<T, SpecToWorkerError>;

pub const WORKER_NAME: &str = "spec-to-worker";
pub const SPEC_TO_WORKER_FUNCTION_IDS: [&str; 1] = ["spec-to-worker::convert"];

#[derive(Debug, thiserror::Error)]
pub enum SpecToWorkerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    OpenApi,
    Graphql,
    Har,
    Mcp,
    Docs,
    Url,
    #[default]
    Manual,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConvertSpecToWorkerInput {
    pub name: Option<String>,
    pub goal: Option<String>,
    #[serde(alias = "source_type")]
    pub source_type: Option<SourceType>,
    #[serde(alias = "url")]
    pub source: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub functions: Vec<String>,
}

#[derive(Debug, Clone)]
struct SpecSource {
    worker_name: String,
    namespace: String,
    source_type: SourceType,
    source: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
    functions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFunctionRegistration {
    pub function_id: String,
    pub url: String,
    pub method: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedWorkerManifest {
    pub schema: String,
    pub worker_name: String,
    pub namespace: String,
    pub source_type: SourceType,
    pub source: Option<String>,
    pub functions: Vec<GeneratedFunctionRegistration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecToWorkerConversion {
    pub ok: bool,
    pub mode: String,
    pub worker_name: String,
    pub namespace: String,
    pub source_type: SourceType,
    pub source: Option<String>,
    pub function_count: usize,
    pub registered_functions: Vec<GeneratedFunctionRegistration>,
    pub manifest: GeneratedWorkerManifest,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct GeneratedHttpFunction {
    function_id: String,
    url: String,
    method: HttpMethod,
    mcp_transport: Option<McpTransport>,
    invocation: GeneratedInvocation,
    description: String,
    request_format: Option<Value>,
    response_format: Option<Value>,
    metadata: Value,
}

#[derive(Debug, Clone)]
enum GeneratedInvocation {
    Http,
    McpTool { tool_name: String },
    McpToolsList,
    McpToolCall,
}

#[derive(Debug, Clone)]
enum McpTransport {
    Http(String),
    Stdio(McpStdioConfig),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpStdioConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpToolSpec {
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "empty_object_schema", alias = "input_schema")]
    input_schema: Value,
    #[serde(default, alias = "output_schema")]
    output_schema: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpListToolsResult {
    #[serde(default)]
    tools: Vec<McpToolSpec>,
}

#[derive(Clone)]
struct BridgeServer {
    base_url: String,
    functions: Arc<Mutex<HashMap<String, GeneratedHttpFunction>>>,
}

struct BridgeRequest {
    path: String,
    body: Vec<u8>,
}

static GENERATED_BRIDGE: OnceLock<Mutex<Option<BridgeServer>>> = OnceLock::new();
static GENERATED_FUNCTION_REFS: OnceLock<Mutex<HashMap<String, FunctionRef>>> = OnceLock::new();

pub fn registered_function_ids() -> Vec<&'static str> {
    SPEC_TO_WORKER_FUNCTION_IDS.to_vec()
}

pub fn worker_metadata() -> WorkerMetadata {
    WorkerMetadata {
        runtime: "rust".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        name: WORKER_NAME.into(),
        os: format!(
            "{} {} ({})",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::FAMILY
        ),
        pid: Some(std::process::id()),
        telemetry: None,
        isolation: None,
    }
}

pub fn init_options() -> InitOptions {
    InitOptions {
        metadata: Some(worker_metadata()),
        ..Default::default()
    }
}

pub fn register_spec_to_worker_primitives(iii: &iii_sdk::III) -> Vec<FunctionRef> {
    iii.register_service(RegisterServiceMessage {
        id: WORKER_NAME.into(),
        name: "Spec-to-Worker".into(),
        description: Some(
            "Convert OpenAPI specs and MCP servers into triggerable iii functions.".into(),
        ),
        parent_service_id: None,
    });

    let convert_iii = iii.clone();
    vec![iii.register_function(
        RegisterFunction::new(
            "spec-to-worker::convert",
            move |input: ConvertSpecToWorkerInput| {
                convert_spec_to_worker_for_iii(&convert_iii, input)
            },
        )
        .description("Convert an OpenAPI spec or MCP server into triggerable iii functions."),
    )]
}

pub fn convert_spec_to_worker_for_iii(
    iii: &iii_sdk::III,
    input: ConvertSpecToWorkerInput,
) -> Result<SpecToWorkerConversion> {
    let spec = input.into_spec_source()?;
    match spec.source_type {
        SourceType::OpenApi => register_openapi_generated_worker(iii, spec),
        SourceType::Mcp => register_mcp_generated_worker(iii, spec),
        other => Err(SpecToWorkerError::InvalidInput(format!(
            "spec-to-worker::convert supports open_api and mcp sources; got {other:?}"
        ))),
    }
}

impl ConvertSpecToWorkerInput {
    fn into_spec_source(self) -> Result<SpecSource> {
        let command = match self.command {
            Some(command) => {
                let command = command.trim().to_string();
                if command.is_empty() {
                    return Err(SpecToWorkerError::InvalidInput(
                        "command cannot be blank for MCP stdio conversion".into(),
                    ));
                }
                Some(command)
            }
            None => None,
        };
        let source = match self.source {
            Some(source) => {
                let source = source.trim().to_string();
                if source.is_empty() {
                    return Err(SpecToWorkerError::InvalidInput(
                        "source/url cannot be blank for spec-to-worker::convert".into(),
                    ));
                }
                Some(source)
            }
            None => None,
        }
        .or_else(|| {
            command
                .as_ref()
                .map(|command| mcp_stdio_source_label(command, &self.args))
        });

        let name = match (self.name, source.as_deref()) {
            (Some(name), _) if !name.trim().is_empty() => name.trim().to_string(),
            (_, Some(source)) => infer_name_from_source(source),
            _ => {
                return Err(SpecToWorkerError::InvalidInput(
                    "provide name or url/source for spec-to-worker::convert".into(),
                ));
            }
        };
        let namespace = slugify(&name);
        let source_type = self
            .source_type
            .or_else(|| command.as_ref().map(|_| SourceType::Mcp))
            .or_else(|| source.as_deref().map(infer_source_type_from_source))
            .unwrap_or_default();

        Ok(SpecSource {
            worker_name: format!("{}-worker", namespace.replace('_', "-")),
            namespace,
            source_type,
            source,
            command,
            args: self.args,
            env: self.env,
            functions: self.functions,
        })
    }
}

fn register_openapi_generated_worker(
    iii: &iii_sdk::III,
    spec_source: SpecSource,
) -> Result<SpecToWorkerConversion> {
    let source = spec_source.source.as_deref().ok_or_else(|| {
        SpecToWorkerError::InvalidInput("OpenAPI conversion requires url/source".into())
    })?;
    let spec_text = fetch_text_for_conversion(source)?;
    let openapi_spec = parse_openapi_spec(&spec_text)?;
    let generated_functions = openapi_generated_functions(&spec_source, source, &openapi_spec)?;
    register_generated_worker(iii, &spec_source, generated_functions, "http_invocation")
}

fn register_mcp_generated_worker(
    iii: &iii_sdk::III,
    spec: SpecSource,
) -> Result<SpecToWorkerConversion> {
    let transport = mcp_transport_from_spec(&spec)?;
    let generated_functions = if spec.functions.is_empty() {
        mcp_discovered_generated_functions(&spec, &transport)?
    } else {
        mcp_named_generated_functions(&spec, &transport)
    };
    register_generated_worker(iii, &spec, generated_functions, "http_invocation")
}

fn register_generated_worker(
    iii: &iii_sdk::III,
    spec: &SpecSource,
    generated_functions: Vec<GeneratedHttpFunction>,
    mode: &str,
) -> Result<SpecToWorkerConversion> {
    let mut registered_functions = Vec::new();
    for function in generated_functions {
        let method_label = http_method_label(&function.method).to_string();
        let invocation_url = register_bridge_function(&function)?;
        let message = RegisterFunctionMessage {
            id: function.function_id.clone(),
            description: Some(function.description.clone()),
            request_format: function.request_format.clone(),
            response_format: function.response_format.clone(),
            metadata: Some(function.metadata.clone()),
            invocation: None,
        };
        let mut refs = generated_function_refs().lock().map_err(|_| {
            SpecToWorkerError::InvalidInput("generated function registry lock poisoned".into())
        })?;
        if let Some(existing) = refs.remove(&function.function_id) {
            existing.unregister();
        }
        let function_ref = iii.register_function_with(
            message,
            HttpInvocationConfig {
                url: invocation_url,
                method: HttpMethod::Post,
                timeout_ms: Some(30_000),
                headers: HashMap::new(),
                auth: None,
            },
        );
        refs.insert(function.function_id.clone(), function_ref);
        drop(refs);

        registered_functions.push(GeneratedFunctionRegistration {
            function_id: function.function_id,
            url: function.url,
            method: method_label,
            description: function.description,
        });
    }

    let manifest = GeneratedWorkerManifest {
        schema: "spec-to-worker.http-invocation.v1".into(),
        worker_name: spec.worker_name.clone(),
        namespace: spec.namespace.clone(),
        source_type: spec.source_type.clone(),
        source: spec.source.clone(),
        functions: registered_functions.clone(),
    };

    Ok(SpecToWorkerConversion {
        ok: true,
        mode: mode.into(),
        worker_name: spec.worker_name.clone(),
        namespace: spec.namespace.clone(),
        source_type: spec.source_type.clone(),
        source: spec.source.clone(),
        function_count: registered_functions.len(),
        registered_functions,
        manifest,
        notes: vec![
            "Registered functions are normal iii functions backed by engine HTTP invocation."
                .into(),
            "The engine lists the converted source as an engine-runtime worker group; no worker process is started."
                .into(),
        ],
    })
}

fn openapi_generated_functions(
    spec_source: &SpecSource,
    source: &str,
    openapi_spec: &Value,
) -> Result<Vec<GeneratedHttpFunction>> {
    let paths = openapi_spec
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| SpecToWorkerError::InvalidInput("OpenAPI spec is missing paths".into()))?;
    let base_url = openapi_base_url(openapi_spec, source)?;
    let mut seen = HashSet::new();
    let mut operations = Vec::new();
    let mut path_entries = paths.iter().collect::<Vec<_>>();
    path_entries.sort_by_key(|(path, _)| *path);

    for (path, path_item) in path_entries {
        let Some(path_item) = path_item.as_object() else {
            continue;
        };
        let path_parameters = path_item
            .get("parameters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut method_entries = path_item.iter().collect::<Vec<_>>();
        method_entries.sort_by_key(|(method, _)| *method);
        for (method, operation) in method_entries {
            let Some(http_method) = openapi_http_method(method) else {
                continue;
            };
            let Some(operation) = operation.as_object() else {
                continue;
            };
            let function_id = unique_function_id(
                &spec_source.namespace,
                operation
                    .get("operationId")
                    .and_then(Value::as_str)
                    .map(slugify)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| slugify(&format!("{method}_{path}"))),
                &mut seen,
            );
            let description = operation
                .get("summary")
                .or_else(|| operation.get("description"))
                .and_then(Value::as_str)
                .unwrap_or("OpenAPI HTTP-invoked function")
                .to_string();
            let parameters = openapi_parameters(&path_parameters, operation);
            operations.push(GeneratedHttpFunction {
                function_id,
                url: join_url_path(&base_url, path),
                method: http_method,
                mcp_transport: None,
                invocation: GeneratedInvocation::Http,
                description,
                request_format: Some(openapi_request_format(&parameters, operation)),
                response_format: openapi_response_format(operation),
                metadata: spec_to_worker_metadata(
                    spec_source,
                    serde_json::json!({
                        "path": path,
                        "method": method.to_uppercase()
                    }),
                ),
            });
        }
    }

    if operations.is_empty() {
        return Err(SpecToWorkerError::InvalidInput(
            "OpenAPI spec did not expose any HTTP operations".into(),
        ));
    }
    Ok(operations)
}

fn mcp_discovered_generated_functions(
    spec: &SpecSource,
    transport: &McpTransport,
) -> Result<Vec<GeneratedHttpFunction>> {
    let source = transport.source_label();
    let tools = mcp_list_tools(transport)?;
    if tools.is_empty() {
        return Err(SpecToWorkerError::InvalidInput(
            "MCP server did not expose any tools".into(),
        ));
    }

    let mut seen = HashSet::new();
    Ok(tools
        .into_iter()
        .map(|tool| {
            let function_id = unique_function_id(&spec.namespace, slugify(&tool.name), &mut seen);
            let description = tool
                .description
                .clone()
                .or_else(|| tool.title.clone())
                .unwrap_or_else(|| format!("Call MCP tool {}.", tool.name));
            GeneratedHttpFunction {
                function_id,
                url: source.clone(),
                method: HttpMethod::Post,
                mcp_transport: Some(transport.clone()),
                invocation: GeneratedInvocation::McpTool {
                    tool_name: tool.name.clone(),
                },
                description,
                request_format: Some(tool.input_schema),
                response_format: tool.output_schema,
                metadata: spec_to_worker_metadata(
                    spec,
                    serde_json::json!({ "mcpTool": tool.name }),
                ),
            }
        })
        .collect())
}

fn mcp_named_generated_functions(
    spec: &SpecSource,
    transport: &McpTransport,
) -> Vec<GeneratedHttpFunction> {
    let source = transport.source_label();
    let mut seen = HashSet::new();
    spec.functions
        .iter()
        .map(|function| {
            let local_name = function_local_name(function, &spec.namespace);
            let local_slug = slugify(&local_name);
            let invocation = match local_slug.as_str() {
                "tools_list" => GeneratedInvocation::McpToolsList,
                "tool_call" => GeneratedInvocation::McpToolCall,
                _ => GeneratedInvocation::McpTool {
                    tool_name: local_name.clone(),
                },
            };
            let metadata = match &invocation {
                GeneratedInvocation::McpTool { tool_name } => {
                    spec_to_worker_metadata(spec, serde_json::json!({ "mcpTool": tool_name }))
                }
                GeneratedInvocation::McpToolsList | GeneratedInvocation::McpToolCall => {
                    spec_to_worker_metadata(spec, serde_json::json!({}))
                }
                GeneratedInvocation::Http => spec_to_worker_metadata(spec, serde_json::json!({})),
            };
            GeneratedHttpFunction {
                function_id: unique_function_id(&spec.namespace, local_slug, &mut seen),
                url: source.clone(),
                method: HttpMethod::Post,
                mcp_transport: Some(transport.clone()),
                invocation,
                description: format!("Call MCP tool {local_name}."),
                request_format: Some(empty_object_schema()),
                response_format: None,
                metadata,
            }
        })
        .collect()
}

fn spec_to_worker_metadata(spec: &SpecSource, extra: Value) -> Value {
    let mut spec_meta = serde_json::Map::new();
    spec_meta.insert("mode".into(), Value::String("http_invocation".into()));
    spec_meta.insert(
        "sourceType".into(),
        Value::String(source_type_label(&spec.source_type).into()),
    );
    if let Some(source) = &spec.source {
        spec_meta.insert("source".into(), Value::String(source.clone()));
    }
    spec_meta.insert("workerName".into(), Value::String(spec.worker_name.clone()));
    spec_meta.insert("namespace".into(), Value::String(spec.namespace.clone()));
    if let Value::Object(extra) = extra {
        for (key, value) in extra {
            spec_meta.insert(key, value);
        }
    }
    serde_json::json!({
        "spec": spec_meta,
        "iii": {
            "generatedWorker": { "name": spec.worker_name }
        }
    })
}

fn source_type_label(source_type: &SourceType) -> &'static str {
    match source_type {
        SourceType::OpenApi => "openapi",
        SourceType::Graphql => "graphql",
        SourceType::Har => "har",
        SourceType::Mcp => "mcp",
        SourceType::Docs => "docs",
        SourceType::Url => "url",
        SourceType::Manual => "manual",
    }
}

impl McpTransport {
    fn source_label(&self) -> String {
        match self {
            McpTransport::Http(url) => url.clone(),
            McpTransport::Stdio(config) => mcp_stdio_source_label(&config.command, &config.args),
        }
    }
}

fn mcp_transport_from_spec(spec: &SpecSource) -> Result<McpTransport> {
    if let Some(command) = spec.command.as_deref().filter(|value| !value.is_empty()) {
        return Ok(McpTransport::Stdio(McpStdioConfig {
            command: command.to_string(),
            args: spec.args.clone(),
            env: spec.env.clone(),
        }));
    }
    let source = spec.source.as_deref().ok_or_else(|| {
        SpecToWorkerError::InvalidInput("MCP conversion requires url/source or command".into())
    })?;
    if source.starts_with("http://") || source.starts_with("https://") {
        return Ok(McpTransport::Http(source.to_string()));
    }
    if let Some(command_line) = source.strip_prefix("stdio:") {
        return mcp_stdio_transport_from_source(command_line);
    }
    Err(SpecToWorkerError::InvalidInput(
        "MCP conversion requires an HTTP endpoint or stdio command".into(),
    ))
}

fn mcp_stdio_transport_from_source(command_line: &str) -> Result<McpTransport> {
    let parts = command_line
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let Some(command) = parts.first().filter(|value| !value.is_empty()) else {
        return Err(SpecToWorkerError::InvalidInput(
            "stdio MCP source requires a command".into(),
        ));
    };
    Ok(McpTransport::Stdio(McpStdioConfig {
        command: command.clone(),
        args: parts.iter().skip(1).cloned().collect(),
        env: HashMap::new(),
    }))
}

fn mcp_stdio_source_label(command: &str, args: &[String]) -> String {
    let mut parts = vec![command.to_string()];
    parts.extend(args.iter().cloned());
    format!("stdio:{}", parts.join(" "))
}

fn mcp_list_tools(transport: &McpTransport) -> Result<Vec<McpToolSpec>> {
    if let McpTransport::Stdio(config) = transport {
        let result = mcp_stdio_json_rpc(config, "tools/list", None)?;
        return serde_json::from_value::<McpListToolsResult>(result)
            .map(|result| result.tools)
            .map_err(|error| {
                SpecToWorkerError::InvalidInput(format!("invalid MCP tools/list: {error}"))
            });
    }
    let session_id = mcp_initialize(transport)?;
    mcp_send_initialized_notification(transport, session_id.as_deref())?;
    let result = mcp_json_rpc(transport, "tools/list", None, session_id.as_deref())?;
    serde_json::from_value::<McpListToolsResult>(result)
        .map(|result| result.tools)
        .map_err(|error| {
            SpecToWorkerError::InvalidInput(format!("invalid MCP tools/list: {error}"))
        })
}

fn mcp_tool_call(transport: &McpTransport, tool_name: &str, arguments: Value) -> Result<Value> {
    if let McpTransport::Stdio(config) = transport {
        return mcp_stdio_json_rpc(
            config,
            "tools/call",
            Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            })),
        );
    }
    let session_id = mcp_initialize(transport)?;
    mcp_send_initialized_notification(transport, session_id.as_deref())?;
    mcp_json_rpc(
        transport,
        "tools/call",
        Some(serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        })),
        session_id.as_deref(),
    )
}

fn mcp_initialize(transport: &McpTransport) -> Result<Option<String>> {
    let params = serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": {
            "name": "spec-to-worker",
            "version": env!("CARGO_PKG_VERSION")
        }
    });
    let (_, session_id) = mcp_json_rpc_with_session(transport, "initialize", Some(params), None)?;
    Ok(session_id)
}

fn mcp_send_initialized_notification(
    transport: &McpTransport,
    session_id: Option<&str>,
) -> Result<()> {
    let notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let (status, text, _) = send_mcp_message(transport, &notification, session_id)?;
    if status >= 400 {
        return Err(SpecToWorkerError::InvalidInput(format!(
            "MCP notifications/initialized HTTP {status}: {text}"
        )));
    }
    Ok(())
}

fn mcp_json_rpc(
    transport: &McpTransport,
    method: &str,
    params: Option<Value>,
    session_id: Option<&str>,
) -> Result<Value> {
    mcp_json_rpc_with_session(transport, method, params, session_id).map(|(value, _)| value)
}

fn mcp_json_rpc_with_session(
    transport: &McpTransport,
    method: &str,
    params: Option<Value>,
    session_id: Option<&str>,
) -> Result<(Value, Option<String>)> {
    let mut request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method
    });
    if let Some(params) = params {
        request["params"] = params;
    }

    let (status, text, next_session_id) = send_mcp_message(transport, &request, session_id)?;
    if status >= 400 {
        return Err(SpecToWorkerError::InvalidInput(format!(
            "MCP {method} HTTP {status}: {text}"
        )));
    }
    let value = parse_mcp_http_body(&text)?;
    if let Some(error) = value.get("error") {
        return Err(SpecToWorkerError::InvalidInput(format!(
            "MCP {method} error: {error}"
        )));
    }
    Ok((
        value.get("result").cloned().unwrap_or(value),
        next_session_id,
    ))
}

fn send_mcp_message(
    transport: &McpTransport,
    body: &Value,
    session_id: Option<&str>,
) -> Result<(u16, String, Option<String>)> {
    match transport {
        McpTransport::Http(url) => send_mcp_http(url, body, session_id),
        McpTransport::Stdio(_) => Err(SpecToWorkerError::InvalidInput(
            "stdio MCP uses a per-call session and cannot send standalone messages".into(),
        )),
    }
}

fn send_mcp_http(
    url: &str,
    body: &Value,
    session_id: Option<&str>,
) -> Result<(u16, String, Option<String>)> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let body_text = serde_json::to_string(body)?;
    let mut request = agent
        .post(url)
        .set("User-Agent", "spec-to-worker/0.1")
        .set("Content-Type", "application/json")
        .set("Accept", "application/json, text/event-stream");
    if let Some(session_id) = session_id {
        request = request.set("Mcp-Session-Id", session_id);
    }

    match request.send_string(&body_text) {
        Ok(response) => {
            let status = response.status();
            let session_id = response.header("Mcp-Session-Id").map(str::to_string);
            let text = response
                .into_string()
                .map_err(|error| SpecToWorkerError::InvalidInput(error.to_string()))?;
            Ok((status, text, session_id))
        }
        Err(ureq::Error::Status(status, response)) => {
            let session_id = response.header("Mcp-Session-Id").map(str::to_string);
            let text = response.into_string().unwrap_or_default();
            Ok((status, text, session_id))
        }
        Err(error) => Err(SpecToWorkerError::InvalidInput(error.to_string())),
    }
}

fn mcp_stdio_json_rpc(
    config: &McpStdioConfig,
    method: &str,
    params: Option<Value>,
) -> Result<Value> {
    let mut child = Command::new(&config.command)
        .args(&config.args)
        .envs(&config.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            SpecToWorkerError::InvalidInput(format!(
                "failed to start MCP stdio command '{}': {error}",
                config.command
            ))
        })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| SpecToWorkerError::InvalidInput("MCP stdio stdin unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SpecToWorkerError::InvalidInput("MCP stdio stdout unavailable".into()))?;
    let mut stdout = BufReader::new(stdout);

    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "spec-to-worker",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    });
    write_mcp_stdio_message(&mut stdin, &initialize)?;
    let _ = read_mcp_stdio_response(&mut stdout, 1)?;

    write_mcp_stdio_message(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )?;

    let mut request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": method
    });
    if let Some(params) = params {
        request["params"] = params;
    }
    write_mcp_stdio_message(&mut stdin, &request)?;
    let result = read_mcp_stdio_response(&mut stdout, 2);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn write_mcp_stdio_message(stdin: &mut impl Write, value: &Value) -> Result<()> {
    let line = serde_json::to_string(value)?;
    stdin.write_all(line.as_bytes())?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn read_mcp_stdio_response(stdout: &mut impl BufRead, expected_id: i64) -> Result<Value> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = stdout.read_line(&mut line)?;
        if read == 0 {
            return Err(SpecToWorkerError::InvalidInput(
                "MCP stdio server closed stdout before response".into(),
            ));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if value.get("id").and_then(Value::as_i64) != Some(expected_id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            return Err(SpecToWorkerError::InvalidInput(format!(
                "MCP stdio error: {error}"
            )));
        }
        return Ok(value.get("result").cloned().unwrap_or(value));
    }
}

fn parse_mcp_http_body(text: &str) -> Result<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    if let Some(data) = first_sse_data(trimmed) {
        return serde_json::from_str::<Value>(&data).map_err(SpecToWorkerError::from);
    }
    Err(SpecToWorkerError::InvalidInput(
        "MCP response was not JSON or SSE JSON".into(),
    ))
}

fn first_sse_data(text: &str) -> Option<String> {
    for event in text.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>();
        if !data.is_empty() {
            return Some(data.join("\n"));
        }
    }
    None
}

fn fetch_text_for_conversion(source: &str) -> Result<String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        return ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .get(source)
            .set("User-Agent", "spec-to-worker/0.1")
            .call()
            .map_err(|error| SpecToWorkerError::InvalidInput(error.to_string()))?
            .into_string()
            .map_err(|error| SpecToWorkerError::InvalidInput(error.to_string()));
    }
    fs::read_to_string(source).map_err(SpecToWorkerError::from)
}

fn parse_openapi_spec(spec_text: &str) -> Result<Value> {
    match serde_json::from_str(spec_text) {
        Ok(spec) => Ok(spec),
        Err(json_error) => serde_yml::from_str(spec_text).map_err(|yaml_error| {
            SpecToWorkerError::InvalidInput(format!(
                "failed to parse OpenAPI as JSON ({json_error}) or YAML ({yaml_error})"
            ))
        }),
    }
}

fn openapi_base_url(spec: &Value, source: &str) -> Result<String> {
    if let Some(server_url) = spec
        .get("servers")
        .and_then(Value::as_array)
        .and_then(|servers| servers.first())
        .and_then(|server| server.get("url"))
        .and_then(Value::as_str)
        .filter(|url| !url.trim().is_empty())
    {
        let server_url = server_url.trim();
        if server_url.starts_with("http://") || server_url.starts_with("https://") {
            return Ok(server_url.trim_end_matches('/').into());
        }
        if let Some(source_origin) = url_origin(source) {
            return Ok(join_url_path(&source_origin, server_url)
                .trim_end_matches('/')
                .into());
        }
    }
    if let Some(host) = spec.get("host").and_then(Value::as_str) {
        let scheme = spec
            .get("schemes")
            .and_then(Value::as_array)
            .and_then(|schemes| schemes.first())
            .and_then(Value::as_str)
            .unwrap_or("https");
        let base_path = spec
            .get("basePath")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Ok(join_url_path(&format!("{scheme}://{host}"), base_path)
            .trim_end_matches('/')
            .into());
    }
    url_origin(source).ok_or_else(|| {
        SpecToWorkerError::InvalidInput(
            "OpenAPI spec needs servers[0].url, host, or an absolute source URL".into(),
        )
    })
}

fn openapi_http_method(method: &str) -> Option<HttpMethod> {
    match method.to_ascii_lowercase().as_str() {
        "get" => Some(HttpMethod::Get),
        "post" => Some(HttpMethod::Post),
        "put" => Some(HttpMethod::Put),
        "patch" => Some(HttpMethod::Patch),
        "delete" => Some(HttpMethod::Delete),
        _ => None,
    }
}

fn openapi_parameters(
    path_parameters: &[Value],
    operation: &serde_json::Map<String, Value>,
) -> Vec<Value> {
    let mut parameters = path_parameters.to_vec();
    if let Some(operation_parameters) = operation.get("parameters").and_then(Value::as_array) {
        parameters.extend(operation_parameters.iter().cloned());
    }
    parameters
}

fn openapi_request_format(
    parameters: &[Value],
    operation: &serde_json::Map<String, Value>,
) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for parameter in parameters {
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };
        properties.insert(
            name.into(),
            parameter
                .get("schema")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({ "type": "string" })),
        );
        if parameter
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required.push(Value::String(name.into()));
        }
    }

    if let Some(body_schema) = operation
        .get("requestBody")
        .and_then(|body| body.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| {
            content
                .get("application/json")
                .or_else(|| content.values().next())
        })
        .and_then(|media| media.get("schema"))
    {
        properties.insert("body".into(), body_schema.clone());
        if operation
            .get("requestBody")
            .and_then(|body| body.get("required"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required.push(Value::String("body".into()));
        }
    }

    let mut schema = serde_json::json!({
        "type": "object",
        "properties": properties
    });
    if !required.is_empty() {
        schema["required"] = Value::Array(required);
    }
    schema
}

fn openapi_response_format(operation: &serde_json::Map<String, Value>) -> Option<Value> {
    operation
        .get("responses")
        .and_then(Value::as_object)
        .and_then(|responses| {
            responses
                .get("200")
                .or_else(|| responses.get("201"))
                .or_else(|| responses.get("default"))
        })
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| {
            content
                .get("application/json")
                .or_else(|| content.values().next())
        })
        .and_then(|media| media.get("schema"))
        .cloned()
}

fn register_bridge_function(function: &GeneratedHttpFunction) -> Result<String> {
    let bridge = bridge_server()?;
    let key = bridge_key(&function.function_id);
    bridge
        .functions
        .lock()
        .map_err(|_| SpecToWorkerError::InvalidInput("bridge registry lock poisoned".into()))?
        .insert(key.clone(), function.clone());
    Ok(format!("{}/invoke/{}", bridge.base_url, key))
}

fn bridge_server() -> Result<BridgeServer> {
    let slot = GENERATED_BRIDGE.get_or_init(|| Mutex::new(None));
    let mut guard = slot
        .lock()
        .map_err(|_| SpecToWorkerError::InvalidInput("bridge server lock poisoned".into()))?;
    if let Some(server) = guard.clone() {
        return Ok(server);
    }

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let functions = Arc::new(Mutex::new(HashMap::new()));
    let server = BridgeServer {
        base_url: format!("http://{addr}"),
        functions: functions.clone(),
    };

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let functions = functions.clone();
            thread::spawn(move || handle_bridge_connection(stream, functions));
        }
    });

    *guard = Some(server.clone());
    Ok(server)
}

fn handle_bridge_connection(
    mut stream: TcpStream,
    functions: Arc<Mutex<HashMap<String, GeneratedHttpFunction>>>,
) {
    let response = match read_bridge_request(&mut stream).and_then(|request| {
        let Some(key) = request.path.strip_prefix("/invoke/") else {
            return Err(SpecToWorkerError::InvalidInput(
                "unknown bridge route".into(),
            ));
        };
        let function = functions
            .lock()
            .map_err(|_| SpecToWorkerError::InvalidInput("bridge registry lock poisoned".into()))?
            .get(key)
            .cloned()
            .ok_or_else(|| SpecToWorkerError::InvalidInput("unknown bridge function".into()))?;
        let payload = if request.body.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_slice(&request.body)?
        };
        invoke_generated_http_function(&function, payload)
    }) {
        Ok(value) => (200, value),
        Err(error) => (
            500,
            serde_json::json!({
                "error": {
                    "code": "spec_bridge_error",
                    "message": error.to_string()
                }
            }),
        ),
    };

    let _ = write_bridge_response(&mut stream, response.0, &response.1);
}

fn read_bridge_request(stream: &mut TcpStream) -> Result<BridgeRequest> {
    let mut buffer = Vec::new();
    let mut temp = [0_u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut temp)?;
        if read == 0 {
            return Err(SpecToWorkerError::InvalidInput(
                "empty bridge request".into(),
            ));
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
        if buffer.len() > 1024 * 1024 {
            return Err(SpecToWorkerError::InvalidInput(
                "bridge request too large".into(),
            ));
        }
    };

    let headers_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| SpecToWorkerError::InvalidInput("missing bridge request line".into()))?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| SpecToWorkerError::InvalidInput("missing bridge request path".into()))?
        .to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut temp)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
    }
    let body = buffer
        .get(body_start..body_start + content_length.min(buffer.len().saturating_sub(body_start)))
        .unwrap_or_default()
        .to_vec();

    Ok(BridgeRequest { path, body })
}

fn invoke_generated_http_function(
    function: &GeneratedHttpFunction,
    payload: Value,
) -> Result<Value> {
    let payload = strip_iii_runtime_fields(payload);
    match &function.invocation {
        GeneratedInvocation::Http => invoke_plain_generated_http_function(function, payload),
        GeneratedInvocation::McpTool { tool_name } => mcp_tool_call(
            function.mcp_transport.as_ref().ok_or_else(|| {
                SpecToWorkerError::InvalidInput("MCP function is missing transport".into())
            })?,
            tool_name,
            payload,
        ),
        GeneratedInvocation::McpToolsList => {
            mcp_list_tools(function.mcp_transport.as_ref().ok_or_else(|| {
                SpecToWorkerError::InvalidInput("MCP function is missing transport".into())
            })?)
            .map(|tools| serde_json::json!({ "tools": tools }))
        }
        GeneratedInvocation::McpToolCall => {
            let tool_name = payload.get("name").and_then(Value::as_str).ok_or_else(|| {
                SpecToWorkerError::InvalidInput("MCP tool_call payload requires name".into())
            })?;
            let arguments = payload
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            mcp_tool_call(
                function.mcp_transport.as_ref().ok_or_else(|| {
                    SpecToWorkerError::InvalidInput("MCP function is missing transport".into())
                })?,
                tool_name,
                arguments,
            )
        }
    }
}

fn invoke_plain_generated_http_function(
    function: &GeneratedHttpFunction,
    payload: Value,
) -> Result<Value> {
    let url = generated_function_url(function, &payload);
    let body = generated_function_body(&payload);
    let (status, text) = send_generated_http(&function.method, &url, &body)?;
    parse_generated_http_response(status, text)
}

fn send_generated_http(method: &HttpMethod, url: &str, body: &Value) -> Result<(u16, String)> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let body_text = serde_json::to_string(body)?;
    let result = match method {
        HttpMethod::Get => agent
            .get(url)
            .set("User-Agent", "spec-to-worker/0.1")
            .call(),
        HttpMethod::Post => agent
            .post(url)
            .set("User-Agent", "spec-to-worker/0.1")
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream")
            .send_string(&body_text),
        HttpMethod::Put => agent
            .put(url)
            .set("User-Agent", "spec-to-worker/0.1")
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream")
            .send_string(&body_text),
        HttpMethod::Patch => agent
            .request("PATCH", url)
            .set("User-Agent", "spec-to-worker/0.1")
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream")
            .send_string(&body_text),
        HttpMethod::Delete => agent
            .delete(url)
            .set("User-Agent", "spec-to-worker/0.1")
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream")
            .send_string(&body_text),
    };
    match result {
        Ok(response) => {
            let status = response.status();
            let text = response
                .into_string()
                .map_err(|error| SpecToWorkerError::InvalidInput(error.to_string()))?;
            Ok((status, text))
        }
        Err(ureq::Error::Status(status, response)) => {
            let text = response.into_string().unwrap_or_default();
            Ok((status, text))
        }
        Err(error) => Err(SpecToWorkerError::InvalidInput(error.to_string())),
    }
}

fn parse_generated_http_response(status: u16, text: String) -> Result<Value> {
    if text.trim().is_empty() {
        return Ok(serde_json::json!({ "ok": status < 400, "status": status }));
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => Ok(value),
        Err(_) => Ok(serde_json::json!({
            "ok": status < 400,
            "status": status,
            "body": text
        })),
    }
}

fn generated_function_url(function: &GeneratedHttpFunction, payload: &Value) -> String {
    let mut url = apply_path_parameters(&function.url, payload);
    if matches!(function.method, HttpMethod::Get) {
        let path_keys = path_parameter_names(&function.url);
        let query = query_string(payload, &path_keys);
        if !query.is_empty() {
            let separator = if url.contains('?') { '&' } else { '?' };
            url.push(separator);
            url.push_str(&query);
        }
    }
    url
}

fn generated_function_body(payload: &Value) -> Value {
    payload
        .get("body")
        .cloned()
        .unwrap_or_else(|| payload.clone())
}

fn apply_path_parameters(url: &str, payload: &Value) -> String {
    let mut rendered = url.to_string();
    let Some(object) = payload.as_object() else {
        return rendered;
    };
    for (key, value) in object {
        let token = format!("{{{key}}}");
        if rendered.contains(&token) {
            rendered = rendered.replace(&token, &query_value(value));
        }
    }
    rendered
}

fn path_parameter_names(url: &str) -> HashSet<String> {
    let mut keys = HashSet::new();
    let mut rest = url;
    while let Some(start) = rest.find('{') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('}') else {
            break;
        };
        let key = after_start[..end].trim();
        if !key.is_empty() {
            keys.insert(key.to_string());
        }
        rest = &after_start[end + 1..];
    }
    keys
}

fn query_string(payload: &Value, path_keys: &HashSet<String>) -> String {
    payload
        .as_object()
        .map(|object| {
            object
                .iter()
                .filter(|(key, value)| {
                    *key != "body" && !path_keys.contains(*key) && is_query_scalar(value)
                })
                .map(|(key, value)| format!("{}={}", percent_encode(key), query_value(value)))
                .collect::<Vec<_>>()
                .join("&")
        })
        .unwrap_or_default()
}

fn is_query_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
    )
}

fn query_value(value: &Value) -> String {
    match value {
        Value::String(value) => percent_encode(value),
        Value::Number(value) => percent_encode(&value.to_string()),
        Value::Bool(value) => percent_encode(if *value { "true" } else { "false" }),
        Value::Null => String::new(),
        other => percent_encode(&other.to_string()),
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            byte => format!("%{byte:02X}").chars().collect::<Vec<_>>(),
        })
        .collect()
}

fn strip_iii_runtime_fields(payload: Value) -> Value {
    let Value::Object(mut object) = payload else {
        return payload;
    };
    object.retain(|key, _| {
        !key.starts_with("_caller_") && !key.starts_with("_iii_") && !key.starts_with("_trace_")
    });
    Value::Object(object)
}

fn write_bridge_response(stream: &mut TcpStream, status: u16, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    let status_text = if status < 400 { "OK" } else { "ERROR" };
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn generated_function_refs() -> &'static Mutex<HashMap<String, FunctionRef>> {
    GENERATED_FUNCTION_REFS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bridge_key(function_id: &str) -> String {
    function_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn empty_object_schema() -> Value {
    serde_json::json!({ "type": "object", "properties": {} })
}

fn http_method_label(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn unique_function_id(namespace: &str, slug: String, seen: &mut HashSet<String>) -> String {
    let base = format!("{}::{}", namespace, slug);
    if seen.insert(base.clone()) {
        return base;
    }
    let mut index = 2;
    loop {
        let candidate = format!("{base}_{index}");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn function_local_name(function_id: &str, namespace: &str) -> String {
    function_id
        .strip_prefix(&format!("{namespace}::"))
        .unwrap_or(function_id)
        .to_string()
}

fn infer_source_type_from_source(source: &str) -> SourceType {
    let lower = source.to_lowercase();
    if lower.starts_with("stdio:") || lower.contains("mcp") {
        SourceType::Mcp
    } else if lower.contains("openapi")
        || lower.contains("swagger")
        || lower.ends_with(".openapi.json")
        || lower.ends_with(".openapi.yaml")
        || lower.ends_with(".openapi.yml")
    {
        SourceType::OpenApi
    } else if lower.starts_with("http://") || lower.starts_with("https://") {
        SourceType::Url
    } else {
        SourceType::Manual
    }
}

fn infer_name_from_source(source: &str) -> String {
    if source.starts_with("stdio:") {
        return "mcp_stdio".into();
    }
    let no_scheme = source
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let no_query = no_scheme.split(['?', '#']).next().unwrap_or(no_scheme);
    let parts = no_query
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.len() >= 5 && parts[0].contains("github.com") && matches!(parts[3], "tree" | "blob") {
        return parts.last().copied().unwrap_or("spec").to_string();
    }
    if parts.len() >= 3 && parts[0].contains("github.com") {
        return format!("{}-{}", parts[1], parts[2]);
    }

    parts
        .last()
        .copied()
        .filter(|part| !part.is_empty())
        .unwrap_or("spec")
        .trim_end_matches(".json")
        .trim_end_matches(".yaml")
        .trim_end_matches(".yml")
        .trim_end_matches(".openapi")
        .to_string()
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_sep = false;
        } else if !last_was_sep && !out.is_empty() {
            out.push('_');
            last_was_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "spec".into()
    } else {
        out
    }
}

fn join_url_path(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim();
    if path.is_empty() {
        base.to_string()
    } else if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

fn url_origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?.to_lowercase();
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(format!("{}://{}", scheme.to_lowercase(), authority))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_generated_function_does_not_duplicate_path_params_in_query() {
        let function = GeneratedHttpFunction {
            function_id: "demo::get_story".into(),
            url: "http://127.0.0.1:18089/stories/{id}".into(),
            method: HttpMethod::Get,
            mcp_transport: None,
            invocation: GeneratedInvocation::Http,
            description: "Get story".into(),
            request_format: None,
            response_format: None,
            metadata: Value::Null,
        };

        let url = generated_function_url(
            &function,
            &serde_json::json!({
                "id": "42",
                "include": "comments"
            }),
        );

        assert_eq!(url, "http://127.0.0.1:18089/stories/42?include=comments");
    }

    #[test]
    fn metadata_carries_private_engine_group_hint_and_public_spec_details() {
        let spec = SpecSource {
            namespace: "docs_mcp".into(),
            worker_name: "docs-mcp-worker".into(),
            source_type: SourceType::Mcp,
            source: Some("stdio:npx -y any-mcp-server".into()),
            command: Some("npx".into()),
            args: vec!["-y".into(), "any-mcp-server".into()],
            env: HashMap::new(),
            functions: vec![],
        };

        let metadata =
            spec_to_worker_metadata(&spec, serde_json::json!({ "mcpTool": "search-docs" }));

        assert_eq!(metadata["spec"]["mode"], "http_invocation");
        assert_eq!(metadata["spec"]["sourceType"], "mcp");
        assert_eq!(metadata["spec"]["namespace"], "docs_mcp");
        assert_eq!(metadata["spec"]["workerName"], "docs-mcp-worker");
        assert_eq!(metadata["spec"]["mcpTool"], "search-docs");
        assert_eq!(
            metadata["iii"]["generatedWorker"]["name"],
            "docs-mcp-worker"
        );
    }
}
