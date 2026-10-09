//! Session-scoped discovery and local validation. No command is sent by preflight.
use super::*;
use sha2::{Digest, Sha256};
use std::time::Instant;

const MAX_DEFINITION_BYTES: usize = 256 * 1024;

pub(super) struct Catalog {
    identity: String,
    generation: u64,
    loaded: Instant,
    tools: BTreeMap<String, Arc<ToolDefinition>>,
}

pub struct ToolDefinition {
    pub value: Value,
    pub fingerprint: String,
    validator: Result<jsonschema::Validator, String>,
}

pub(crate) struct BoundTool {
    pub definition: Arc<ToolDefinition>,
    pub binding: SessionBinding,
}

impl ToolDefinition {
    pub(crate) fn valid_schema(&self) -> Result<(), McpError> {
        self.validator
            .as_ref()
            .map(|_| ())
            .map_err(|e| McpError::Preflight(e.clone()))
    }
    pub(crate) fn validate_output(&self, value: &Value) -> Result<(), McpError> {
        let validator = self.output_validator()?;
        validator.validate(value).map_err(|e| {
            McpError::Preflight(format!(
                "Invalid device observation at {}: {e}",
                e.instance_path()
            ))
        })
    }
    pub(crate) fn output_validator(&self) -> Result<jsonschema::Validator, McpError> {
        let schema = self
            .value
            .get("outputSchema")
            .ok_or_else(|| McpError::Preflight("Mint Device v1 requires outputSchema".into()))?;
        jsonschema::options()
            .with_retriever(DenyExternalReferences)
            .build(schema)
            .map_err(|e| McpError::Preflight(format!("Invalid device outputSchema: {e}")))
    }
    pub fn validate(&self, arguments: &Value) -> Result<(), McpError> {
        let validator = self
            .validator
            .as_ref()
            .map_err(|e| McpError::Preflight(e.clone()))?;
        if !arguments.is_object() {
            return Err(McpError::Preflight("arguments must be an object".into()));
        }
        let errors: Vec<String> = validator
            .iter_errors(arguments)
            .take(8)
            .map(|e| format!("{}: {}", e.instance_path(), e))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(McpError::Preflight(errors.join("; ")))
        }
    }
}

pub(crate) fn tool_timeout(config: &MintConfig, server: &str) -> Result<u64, McpError> {
    let servers = configured_mcp_servers(config)?;
    let entry = servers
        .get(server)
        .ok_or_else(|| McpError::MissingServer(server.into()))?;
    if entry.disabled {
        close_mcp_session(server);
        return Err(McpError::Disabled(server.into()));
    }
    Ok(entry.timeout_secs.unwrap_or(300))
}

pub async fn describe_tool(
    config: &MintConfig,
    server: &str,
    tool: &str,
    chat: &str,
) -> Result<Arc<ToolDefinition>, McpError> {
    Ok(resolve_tool(config, server, tool, chat, None)
        .await?
        .definition)
}

pub(crate) async fn resolve_tool(
    config: &MintConfig,
    server: &str,
    tool: &str,
    chat: &str,
    pinned: Option<&SessionBinding>,
) -> Result<BoundTool, McpError> {
    let (tools, binding) = calls::McpScope::new(chat)
        .run(catalog_tools(config, server, chat, pinned), MCP_TIMEOUT)
        .await?;
    let definition = tools.get(tool).cloned().ok_or_else(|| {
        McpError::Preflight(format!(
            "Unknown tool '{server}/{tool}'. Read mcp_list_tools; do not guess tool names."
        ))
    })?;
    Ok(BoundTool {
        definition,
        binding,
    })
}

pub async fn list_tools(config: &MintConfig, server: &str, chat: &str) -> Result<Value, McpError> {
    let (tools, _) = calls::McpScope::new(chat)
        .run(catalog_tools(config, server, chat, None), MCP_TIMEOUT)
        .await?;
    Ok(json!({"tools":tools.values().map(|t|t.value.clone()).collect::<Vec<_>>()}))
}

async fn catalog_tools(
    config: &MintConfig,
    server: &str,
    chat: &str,
    pinned: Option<&SessionBinding>,
) -> Result<(BTreeMap<String, Arc<ToolDefinition>>, SessionBinding), McpError> {
    let cfg = config.clone();
    let name = server.to_owned();
    let pinned = pinned.cloned();
    let (cache, generation, binding) = tokio::task::spawn_blocking(move || {
        let session = if let Some(binding) = &pinned {
            binding.check()?;
            binding.session.clone()
        } else {
            get_or_start_session(&cfg, &name)?
        };
        let locked = session.lock().unwrap();
        let generation = match &locked.backend {
            McpSessionBackend::Stdio(s) => Arc::clone(&s.catalog_generation),
            McpSessionBackend::Remote(s) => Arc::clone(&s.catalog_generation),
        };
        let binding = SessionBinding {
            server: name,
            session: session.clone(),
            generation: generation.load(Ordering::Acquire),
            generation_counter: Arc::clone(&generation),
        };
        Ok::<_, McpError>((Arc::clone(&locked.catalog), generation, binding))
    })
    .await
    .map_err(|e| McpError::Remote(e.to_string()))??;
    let mut cached = cache.lock().await;
    let epoch = generation.load(Ordering::Acquire);
    if let Some(catalog) = &*cached
        && catalog.generation == epoch
        && catalog.loaded.elapsed() < Duration::from_secs(60)
    {
        binding.check()?;
        return Ok((catalog.tools.clone(), binding));
    }
    let identity = cached
        .as_ref()
        .map(|c| c.identity.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut cursor = None;
    let mut cursors = std::collections::HashSet::new();
    let mut tools = BTreeMap::new();
    for page in 0..100 {
        let response = calls::request_on_session(
            &binding,
            "tools/list",
            json!({"cursor":cursor})
                .as_object()
                .map(|m| {
                    let mut m = m.clone();
                    if cursor.is_none() {
                        m.remove("cursor");
                    }
                    Value::Object(m)
                })
                .unwrap(),
            chat,
            MCP_TIMEOUT,
            |_| {},
        )
        .await?;
        let entries = response["tools"]
            .as_array()
            .ok_or_else(|| McpError::Preflight("tools/list did not return a tools array".into()))?;
        for entry in entries {
            let name = entry["name"]
                .as_str()
                .filter(|n| !n.is_empty())
                .ok_or_else(|| McpError::Preflight("tool definition has no name".into()))?;
            let serialized = serde_json::to_string(entry).unwrap();
            if serialized.len() > MAX_DEFINITION_BYTES || tools.len() >= 1000 {
                return Err(McpError::Preflight("Discovery exceeds 1,000 tools or 256 KiB/definition; no partial schema will be used".into()));
            }
            let validator = entry
                .get("inputSchema")
                .ok_or_else(|| "Missing inputSchema".to_string())
                .and_then(|schema| {
                    if schema["type"] != "object" {
                        return Err("inputSchema must declare an object".into());
                    }
                    jsonschema::options()
                        .with_retriever(DenyExternalReferences)
                        .build(schema)
                        .map_err(|e| format!("Invalid or unsupported inputSchema: {e}"))
                });
            let fingerprint = format!("{:x}", Sha256::digest(format!("{identity}:{serialized}")));
            if tools
                .insert(
                    name.into(),
                    Arc::new(ToolDefinition {
                        value: entry.clone(),
                        fingerprint,
                        validator,
                    }),
                )
                .is_some()
            {
                return Err(McpError::Preflight(format!("Duplicate tool name: {name}")));
            }
        }
        cursor = match response.get("nextCursor") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => return Err(McpError::Preflight("Invalid pagination cursor".into())),
        };
        if cursor.is_none() {
            break;
        }
        if page == 99 || !cursors.insert(cursor.clone().unwrap()) {
            return Err(McpError::Preflight(
                "Discovery pagination exceeded its bound or repeated a cursor".into(),
            ));
        }
    }
    if generation.load(Ordering::Acquire) != epoch {
        return Err(McpError::Preflight(
            "Tool definitions changed during discovery; prepare again before calling".into(),
        ));
    }
    let result = tools.clone();
    *cached = Some(Catalog {
        identity,
        generation: epoch,
        loaded: Instant::now(),
        tools,
    });
    binding.check()?;
    Ok((result, binding))
}

struct DenyExternalReferences;
impl jsonschema::Retrieve for DenyExternalReferences {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(format!("External schema reference is disabled: {uri}").into())
    }
}
