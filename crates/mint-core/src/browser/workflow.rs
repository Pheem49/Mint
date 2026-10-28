//! Bounded command-line workflows using the same session engine as Mint Browser.
use crate::MintConfig;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserWorkflow {
    pub steps: Vec<BrowserStep>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserStep {
    pub action: String,
    #[serde(default)]
    pub input: Value,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowImage {
    pub path: PathBuf,
    pub metadata: Value,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowReport {
    pub version: u32,
    pub status: String,
    pub steps: Vec<Value>,
    pub screenshots: Vec<WorkflowImage>,
    pub error: Option<String>,
}

pub fn validate(
    workflow: &BrowserWorkflow,
    config: &MintConfig,
    plan_mode: bool,
) -> Result<(), String> {
    if workflow.steps.is_empty() || workflow.steps.len() > 100 {
        return Err("A browser workflow must contain between 1 and 100 steps".into());
    }
    for step in &workflow.steps {
        if !super::lifecycle::BROWSER_TOOLS.contains(&step.action.as_str())
            || config.disabled_tools.contains(&step.action)
            || config.extra.get("browserAutomationEnabled") == Some(&json!(false))
        {
            return Err(format!(
                "Browser action '{}' is unavailable or disabled",
                step.action
            ));
        }
        if plan_mode
            && !crate::prompts::agent::PLAN_MODE_ALLOWED_ACTIONS.contains(&step.action.as_str())
        {
            return Err(format!(
                "Browser action '{}' is not allowed in Plan Mode",
                step.action
            ));
        }
        if !step.input.is_null() && !step.input.is_object() {
            return Err("Browser step input must be an object".into());
        }
        if step.action == "browser_open" {
            super::navigate::validate_url(step.input["url"].as_str().unwrap_or(""))?;
        }
    }
    Ok(())
}

pub async fn run(
    config: &MintConfig,
    workflow: BrowserWorkflow,
    output: &Path,
    plan_mode: bool,
    mut log: impl FnMut(&str),
) -> Result<WorkflowReport, String> {
    validate(&workflow, config, plan_mode)?;
    std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let output = output.canonicalize().map_err(|e| e.to_string())?;
    let mut report = WorkflowReport {
        version: 1,
        status: "succeeded".into(),
        steps: Vec::new(),
        screenshots: Vec::new(),
        error: None,
    };
    let result=tokio::time::timeout(Duration::from_secs(120),super::run_session(async {
        for (index,step) in workflow.steps.into_iter().enumerate() {
            log(&format!("Step {}: {}",index+1,step.action));
            let mut input=if step.input.is_null(){json!({})}else{step.input};
            if let Some(ms)=input["timeoutMs"].as_u64(){input["timeoutMs"]=ms.clamp(1,30_000).into();}
            let result=tokio::time::timeout(Duration::from_secs(31),super::execute_action(config,&step.action,&input)).await;
            let result=match result {Ok(result)=>result,Err(_)=>Err("Action timed out; input may already have been dispatched. Observe before trying again.".into())};
            match result {
                Ok(text)=>{
                    if step.action=="browser_screenshot" {
                        let bytes=STANDARD.decode(text.strip_prefix("data:image/png;base64,").ok_or("Screenshot returned no PNG")?).map_err(|e|e.to_string())?;
                        crate::media::tool_images::prepare(&bytes,"image/png")?;
                        let path=output.join(format!("screenshot-{}.png",index+1));
                        std::fs::write(&path,&bytes).map_err(|e|e.to_string())?;
                        report.screenshots.push(WorkflowImage{path:path.clone(),metadata:super::capture_metadata().await});
                        report.steps.push(json!({"step":index+1,"action":step.action,"status":"succeeded","path":path}));
                        log(&format!("Screenshot saved: {}",path.display()));
                    }else{
                        let result=serde_json::from_str::<Value>(&text).unwrap_or(json!({"text":text}));
                        if let Some(command)=result["runtime"]["command"].as_str(){log(&format!("Ran {command}"));}
                        if let Some(diagnostics)=result["runtime"]["diagnostics"].as_str().filter(|s|!s.is_empty()){log(diagnostics);}
                        let failure=result.get("error").filter(|e|!e.is_null());
                        let error=failure.map(|e|e.to_string());
                        report.steps.push(json!({"step":index+1,"action":step.action,"status":if error.is_some(){"failed"}else{"succeeded"},"result":result}));
                        if let Some(error)=error {report.status="failed".into();report.error=Some(error);break;}
                    }
                }
                Err(error)=>{report.steps.push(json!({"step":index+1,"action":step.action,"status":"failed","error":error}));report.status="failed".into();report.error=Some(error);break;}
            }
            let mut checkpoint=serde_json::to_value(&report).map_err(|e|e.to_string())?;
            checkpoint["status"]="running".into();
            std::fs::write(output.join("report.json"),serde_json::to_vec(&checkpoint).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        }
        Ok::<(),String>(())
    })).await;
    match result {
        Err(_) => {
            report.status = "timed_out".into();
            report.error = Some("Workflow exceeded 120 seconds; completion is unconfirmed.".into());
        }
        Ok(Err(error)) => {
            report.status = "failed".into();
            report.error = Some(error);
        }
        Ok(Ok(())) => {}
    }
    Ok(report)
}

/// A direct Mint helper command has a private report channel. Arbitrary shell
/// output is never interpreted as attachment metadata.
pub(crate) struct ShellBridge {
    directory: PathBuf,
}
impl ShellBridge {
    pub(crate) fn prepare(
        command: &str,
        root: &Path,
        config: &mut MintConfig,
    ) -> Result<Option<Self>, String> {
        if command
            .chars()
            .any(|c| matches!(c, ';' | '|' | '&' | '`' | '$' | '\n' | '>' | '<'))
        {
            return Ok(None);
        }
        let Some(tokens) = shlex::split(command) else {
            return Ok(None);
        };
        if tokens.len() < 4 || tokens[1] != "browser" || tokens[2] != "run" {
            return Ok(None);
        }
        let executable = Path::new(&tokens[0])
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !matches!(executable, "mint" | "mint.exe") {
            return Ok(None);
        }
        let root = crate::assert_path_capability(root, crate::Capability::Write, config)
            .map_err(|e| e.to_string())?;
        let directory = root
            .join(".mint/browser-runs")
            .join(uuid::Uuid::new_v4().to_string());
        let mut policy = json!({"disabledTools":config.disabled_tools,"allowedReadPaths":config.allowed_read_paths,"allowedWritePaths":config.allowed_write_paths,"safetyEnabled":config.safety_enabled});
        for key in [
            "browserAutomationEnabled",
            "browserAgentMode",
            "browserExecutable",
            "automationBrowser",
            "browserViewportWidth",
            "browserViewportHeight",
            "browserLaunchArgs",
            "browserDebugUrl",
            "browserUseExisting",
        ] {
            if let Some(value) = config.extra.get(key) {
                policy[key] = value.clone();
            }
        }
        config.extra.insert("__browserWorkflowEnv".into(),json!({"MINT_BROWSER_REPORT":directory.join("report.json"),"MINT_BROWSER_POLICY":policy.to_string()}));
        Ok(Some(Self { directory }))
    }
    pub(crate) fn images(
        &self,
        chat: &str,
        call: &str,
    ) -> Result<(crate::mcp_result::ToolOutcome, bool), String> {
        let path = self.directory.join("report.json");
        let bytes = std::fs::read(&path).map_err(|_| {
            "Browser workflow did not write its report; screenshots are unavailable.".to_string()
        })?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Browser report exceeds its size limit".into());
        }
        let report: WorkflowReport = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if report.version != 1 || report.screenshots.len() > 100 {
            return Err("Invalid browser workflow report".into());
        }
        let mut verified = false;
        for step in &report.steps {
            if matches!(
                step["action"].as_str(),
                Some(
                    "browser_open"
                        | "browser_click"
                        | "browser_type"
                        | "browser_fill"
                        | "browser_key_press"
                        | "browser_select"
                        | "browser_tabs"
                        | "browser_wait"
                )
            ) {
                verified = step["result"]["verification"] == "verified";
            }
        }
        verified = verified && report.status == "succeeded";
        let directory = self.directory.canonicalize().map_err(|e| e.to_string())?;
        let mut out = crate::mcp_result::ToolOutcome::default();
        for image in report.screenshots {
            let path = image.path.canonicalize().map_err(|e| e.to_string())?;
            if !path.starts_with(&directory)
                || std::fs::metadata(&path).map_err(|e| e.to_string())?.len()
                    > crate::media::tool_images::ORIGINAL_LIMIT as u64
            {
                return Err("Screenshot is outside this workflow or exceeds its limit".into());
            }
            let image = crate::media::tool_images::save(
                &std::fs::read(&path).map_err(|e| e.to_string())?,
                "image/png",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("screenshot.png"),
                chat,
                call,
                "browser",
                Some(image.metadata),
            )?;
            out.text.push_str(&format!("\n{}", image.text));
            out.images.extend(image.images);
            out.artifacts.extend(image.artifacts);
            out.warnings.extend(image.warnings);
        }
        // The parent now owns durable copies; standalone CLI runs keep their
        // workspace files because they never enter this import path.
        let _ = std::fs::remove_dir_all(&self.directory);
        Ok((out, verified))
    }
}

pub(crate) fn configure_shell(command: &mut std::process::Command, config: &MintConfig) {
    if let Some(environment) = config
        .extra
        .get("__browserWorkflowEnv")
        .and_then(Value::as_object)
    {
        for key in ["MINT_BROWSER_REPORT", "MINT_BROWSER_POLICY"] {
            if let Some(value) = environment.get(key).and_then(Value::as_str) {
                command.env(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires local Chrome and cargo build -p mint-cli"]
    fn actual_cli_private_report_imports_screenshots_and_preserves_originals() {
        use std::{
            io::{Read, Write},
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let server = std::thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                if let Ok((mut socket, _)) = listener.accept() {
                    let _ = socket.set_read_timeout(Some(Duration::from_secs(1)));
                    let mut request = [0; 4096];
                    let _ = socket.read(&mut request);
                    let body = "<!doctype html><h1>Ready</h1><p>Actual CLI image</p>";
                    let _ = write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        });
        let root =
            std::env::temp_dir().join(format!("mint-real-cli-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = MintConfig::default();
        config.safety_enabled = false;
        config.sandbox_mode = "off".into();
        config.allowed_read_paths.push(root.clone());
        config.allowed_write_paths.push(root.clone());
        config
            .extra
            .insert("browserLaunchArgs".into(), json!(["--no-sandbox"]));
        let config_file = root.join("config.json");
        std::fs::write(&config_file, serde_json::to_vec(&config).unwrap()).unwrap();
        std::fs::write(root.join("task.json"),json!({"steps":[{"action":"browser_open","input":{"url":url}},{"action":"browser_wait","input":{"condition":"text","value":"Ready"}},{"action":"browser_screenshot","input":{}}]}).to_string()).unwrap();
        let binary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/mint")
            .canonicalize()
            .unwrap();
        let command = format!(
            "{} browser run --file task.json",
            shlex::try_quote(binary.to_str().unwrap()).unwrap()
        );
        let bridge = ShellBridge::prepare(&command, &root, &mut config)
            .unwrap()
            .unwrap();
        let mut process = std::process::Command::new(&binary);
        process
            .current_dir(&root)
            .args(["browser", "run", "--file", "task.json"])
            .env("MINT_CONFIG_PATH", &config_file);
        configure_shell(&mut process, &config);
        let result = process.output().unwrap();
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("--headless=new"));
        let owner = format!("cli-import-{}", uuid::Uuid::new_v4());
        let (out, verified) = bridge.images(&owner, "real-call").unwrap();
        assert!(verified);
        assert_eq!(out.images.len(), 1);
        assert_eq!(out.artifacts.len(), 1);
        let (entry, bytes) = crate::mcp_result::read_artifact(&out.artifacts[0].id).unwrap();
        assert_eq!(entry.chat_id, owner);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert!(!bridge.directory.exists());
        crate::mcp_result::delete_chat_artifacts(&owner).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn workflow_rejects_shell_actions_disabled_tools_and_plan_mutations() {
        let workflow = BrowserWorkflow {
            steps: vec![BrowserStep {
                action: "browser_click".into(),
                input: json!({"selector":"#submit"}),
            }],
        };
        let mut config = MintConfig::default();
        assert!(validate(&workflow, &config, false).is_ok());
        assert!(validate(&workflow, &config, true).is_err());
        config.disabled_tools.push("browser_click".into());
        assert!(validate(&workflow, &config, false).is_err());
        let invalid = BrowserWorkflow {
            steps: vec![BrowserStep {
                action: "run_shell".into(),
                input: json!({"command":"anything"}),
            }],
        };
        assert!(validate(&invalid, &MintConfig::default(), false).is_err());
    }
}
