//! Opt-in Mint Device v1 orchestration; transports and physical I/O stay separate.
use super::*;
use crate::integrations::mcp_result::ToolStatus;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevicePhase {
    Accepted,
    Running,
    Verified,
    Failed,
    Unconfirmed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceUpdate {
    pub state: DevicePhase,
    pub device_id: Option<String>,
    pub operation_id: Option<String>,
    pub target: Value,
    pub actual: Value,
    pub observation_seq: Option<u64>,
    pub elapsed_secs: u64,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    device_id: String,
    operation_id: String,
    state: String,
    target: serde_json::Map<String, Value>,
    actual: serde_json::Map<String, Value>,
    observation_seq: u64,
    error: Option<String>,
}

impl Observation {
    fn read(value: &Value, definition: &catalog::ToolDefinition) -> Result<Self, McpError> {
        definition.validate_output(value)?;
        let result: Self = serde_json::from_value(value.clone())
            .map_err(|e| McpError::Preflight(format!("Invalid Mint Device v1 observation: {e}")))?;
        if result.device_id.is_empty()
            || result.operation_id.is_empty()
            || !matches!(
                result.state.as_str(),
                "accepted" | "running" | "completed" | "failed"
            )
        {
            return Err(McpError::Preflight(
                "Invalid device identity or state".into(),
            ));
        }
        Ok(result)
    }
    fn update(&self, state: DevicePhase, elapsed_secs: u64, message: String) -> DeviceUpdate {
        DeviceUpdate {
            state,
            device_id: Some(self.device_id.clone()),
            operation_id: Some(self.operation_id.clone()),
            target: Value::Object(self.target.clone()),
            actual: Value::Object(self.actual.clone()),
            observation_seq: Some(self.observation_seq),
            elapsed_secs,
            message,
        }
    }
}

pub(crate) fn status_tool(
    definition: &catalog::ToolDefinition,
) -> Result<Option<String>, McpError> {
    let Some(meta) = definition.value.pointer("/_meta/mint~1device") else {
        return Ok(None);
    };
    if meta["version"] != 1 {
        return Err(McpError::Preflight(
            "Unsupported Mint Device contract version".into(),
        ));
    }
    let status = meta["statusTool"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| McpError::Preflight("Mint Device command has no statusTool".into()))?;
    Ok(Some(status.into()))
}

pub(crate) struct DeviceResult {
    pub receipt: Value,
    pub status: ToolStatus,
    pub message: String,
    pub completion_unconfirmed: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn call_verified(
    config: &MintConfig,
    server: &str,
    tool: &str,
    arguments: Value,
    chat: &str,
    definition: Arc<catalog::ToolDefinition>,
    status_name: &str,
    status_definition: Arc<catalog::ToolDefinition>,
    mut progress: impl FnMut(McpProgress) + Send,
    mut state: impl FnMut(DeviceUpdate) + Send,
) -> DeviceResult {
    let started = Instant::now();
    let timeout = Duration::from_secs(catalog::tool_timeout(config, server).unwrap_or(300));
    let deadline = tokio::time::Instant::now() + timeout;
    let mut receipt = Value::Null;
    let mut last = None::<Observation>;
    let mut command_attempted = false;
    let work = async {
        let current = catalog::resolve_tool(config, server, tool, chat, None).await?;
        if current.definition.fingerprint != definition.fingerprint {
            return Err(McpError::Preflight(
                "Definition changed; read it again before sending the command".into(),
            ));
        }
        let current_status =
            catalog::resolve_tool(config, server, status_name, chat, Some(&current.binding))
                .await?;
        if current_status.definition.fingerprint != status_definition.fingerprint
            || !current.binding.same_session(&current_status.binding)
        {
            return Err(McpError::Preflight(
                "Status definition changed before command dispatch; read it again".into(),
            ));
        }
        command_attempted = true;
        receipt = calls::call_bound_tool(
            config,
            server,
            tool,
            arguments.clone(),
            chat,
            &current,
            &mut progress,
        )
        .await?;
        if receipt["isError"] == true {
            return Err(McpError::Tool(receipt.clone()));
        }
        let accepted = Observation::read(&receipt["structuredContent"], &definition)?;
        if arguments["deviceId"].as_str() != Some(accepted.device_id.as_str()) {
            return Err(McpError::Preflight(
                "Receipt deviceId does not match the command".into(),
            ));
        }
        if accepted.state == "failed" {
            state(
                accepted.update(
                    DevicePhase::Failed,
                    started.elapsed().as_secs(),
                    accepted
                        .error
                        .clone()
                        .unwrap_or_else(|| "Device reported physical failure".into()),
                ),
            );
            last = Some(accepted);
            return Err(McpError::Remote("Device reported physical failure".into()));
        }
        let status_arguments =
            json!({"deviceId":accepted.device_id,"operationId":accepted.operation_id});
        status_definition.validate(&status_arguments)?;
        state(accepted.update(
            DevicePhase::Accepted,
            started.elapsed().as_secs(),
            "Command accepted; physical completion is not yet confirmed".into(),
        ));
        let acceptance_seq = accepted.observation_seq;
        let device = accepted.device_id.clone();
        let operation = accepted.operation_id.clone();
        let target = accepted.target.clone();
        last = Some(accepted);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(McpError::Timeout);
            }
            let response = tokio::time::timeout(
                remaining.min(Duration::from_secs(10)),
                calls::call_bound_tool(
                    config,
                    server,
                    status_name,
                    status_arguments.clone(),
                    chat,
                    &current_status,
                    &mut progress,
                ),
            )
            .await
            .map_err(|_| McpError::Timeout)??;
            if response["isError"] == true {
                return Err(McpError::Tool(response));
            }
            let observed = Observation::read(&response["structuredContent"], &status_definition)?;
            if observed.device_id != device
                || observed.operation_id != operation
                || observed.target != target
            {
                return Err(McpError::Preflight("Status does not match this device, operationId, and target; completion is unconfirmed".into()));
            }
            if observed.observation_seq > acceptance_seq
                && observed.observation_seq > last.as_ref().unwrap().observation_seq
            {
                if observed.state == "failed" {
                    state(
                        observed.update(
                            DevicePhase::Failed,
                            started.elapsed().as_secs(),
                            observed
                                .error
                                .clone()
                                .unwrap_or_else(|| "Device reported physical failure".into()),
                        ),
                    );
                    last = Some(observed);
                    return Err(McpError::Remote("Device reported physical failure".into()));
                }
                if observed.state == "completed" {
                    state(observed.update(
                        DevicePhase::Verified,
                        started.elapsed().as_secs(),
                        "Fresh device observation confirmed completion".into(),
                    ));
                    receipt["structuredContent"] = response["structuredContent"].clone();
                    last = Some(observed);
                    return Ok(());
                }
                state(observed.update(
                    DevicePhase::Running,
                    started.elapsed().as_secs(),
                    "Device is working; awaiting a fresh completed observation".into(),
                ));
                last = Some(observed);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };
    let result = calls::McpScope::new(chat).run(work, timeout).await;
    match result {
        Ok(()) => DeviceResult {
            receipt,
            status: ToolStatus::Success,
            message: "Device completion verified from a fresh observation".into(),
            completion_unconfirmed: false,
        },
        Err(error) => {
            let error = if !receipt.is_null() {
                match error {
                    McpError::Preflight(reason) => {
                        McpError::Remote(format!("Device verification failed: {reason}"))
                    }
                    other => other,
                }
            } else {
                error
            };
            let physical_failure = last.as_ref().is_some_and(|o| o.state == "failed");
            let completion_unconfirmed = command_attempted
                && !physical_failure
                && !matches!(&error, McpError::Preflight(_) | McpError::NotAllowed { .. });
            let message = if physical_failure {
                format!(
                    "Device reported physical failure: {}",
                    last.as_ref()
                        .unwrap()
                        .error
                        .as_deref()
                        .unwrap_or("unknown failure")
                )
            } else {
                format!(
                    "{error}; remote completion is unconfirmed. Do not automatically repeat the command. Canceling a wait does not physically stop the device."
                )
            };
            if !physical_failure {
                let update = last
                    .as_ref()
                    .map(|o| {
                        o.update(
                            DevicePhase::Unconfirmed,
                            started.elapsed().as_secs(),
                            message.clone(),
                        )
                    })
                    .unwrap_or(DeviceUpdate {
                        state: DevicePhase::Unconfirmed,
                        device_id: arguments["deviceId"].as_str().map(str::to_owned),
                        operation_id: None,
                        target: Value::Null,
                        actual: Value::Null,
                        observation_seq: None,
                        elapsed_secs: started.elapsed().as_secs(),
                        message: message.clone(),
                    });
                state(update);
            }
            let status = match error {
                McpError::Timeout | McpError::TimeoutUndelivered => ToolStatus::TimedOut,
                McpError::Cancelled | McpError::CancelledUndelivered => ToolStatus::Cancelled,
                _ => ToolStatus::Failed,
            };
            DeviceResult {
                receipt,
                status,
                message,
                completion_unconfirmed,
            }
        }
    }
}
