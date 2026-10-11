//! Portable single-file mod compiler and restricted QuickJS execution engine.
use rquickjs::{Context, Function, Module, Object, Promise, Runtime, function::Func};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

pub const SDK_VERSION: u32 = 1;
pub const MAX_SOURCE: usize = 512 * 1024;
mod testing;
pub use testing::run_tests;

pub fn unavailable_usage() -> Value {
    json!({"turnId":0,"totalTokens":null,"contextUsedTokens":null,"contextWindowTokens":null,"contextPercent":null,"contextInputTokens":null,"contextOutputTokens":null,"contextStatus":"unavailable","contextWindowSource":"mint_config","provider":"","model":"","lastTurnDelta":null,"compacted":false,
        "context":{"tokens":null,"window":null,"percent":null,"basis":"input","lastTurnDelta":null},"contextIncludingOutput":{"tokens":null,"window":null,"percent":null,"basis":"input_output","lastTurnDelta":null}})
}
pub fn unavailable_viewport() -> Value {
    json!({"surface":null,"columns":null,"rows":null,"widthPx":null,"heightPx":null,"sidePanelOpen":false})
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub sdk_version: u32,
    pub entry: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}
impl Manifest {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.id)
            || self.name.is_empty()
            || self.name.len() > 120
            || self.version.is_empty()
            || self.version.len() > 48
        {
            return Err("Invalid mod ID, name or version".into());
        }
        if self.sdk_version != SDK_VERSION {
            return Err("Unsupported SDK version".into());
        }
        if !matches!(self.entry.as_str(), "main.ts" | "main.js") {
            return Err("Entry must be main.ts or main.js".into());
        }
        if self
            .permissions
            .iter()
            .any(|p| !matches!(p.as_str(), "workspace.read" | "tools.request"))
        {
            return Err("Unknown permission".into());
        }
        Ok(())
    }
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
pub fn content_hash(manifest: &Manifest, source: &str) -> String {
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(manifest).unwrap());
    h.update([0]);
    h.update(source);
    format!("{:x}", h.finalize())
}
pub fn validate_package(manifest: &Value, source: &str) -> Result<String, String> {
    let manifest: Manifest = serde_json::from_value(manifest.clone()).map_err(|e| e.to_string())?;
    manifest.validate()?;
    compile(&manifest, source)
}
pub fn compile(manifest: &Manifest, source: &str) -> Result<String, String> {
    use oxc::{
        allocator::Allocator,
        ast::ast::Statement,
        codegen::Codegen,
        parser::Parser,
        semantic::SemanticBuilder,
        span::SourceType,
        transformer::{TransformOptions, Transformer},
    };
    manifest.validate()?;
    if source.len() > MAX_SOURCE {
        return Err("Mod source exceeds 512 KiB".into());
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        source,
        SourceType::from_path(&manifest.entry).map_err(|e| e.to_string())?,
    )
    .parse();
    if !parsed.errors.is_empty() {
        return Err(format!("Syntax error: {:?}", parsed.errors));
    }
    let mut program = parsed.program;
    for statement in &program.body {
        if let Statement::ImportDeclaration(import) = statement {
            if import.source.value.as_str() != "@mint/mods" {
                return Err("Only @mint/mods imports are supported".into());
            }
        }
        if matches!(statement, Statement::ExportAllDeclaration(_)) {
            return Err("Re-exports are not supported".into());
        }
        if let Statement::ExportNamedDeclaration(export) = statement {
            if export.source.is_some() {
                return Err("Re-exports are not supported".into());
            }
        }
    }
    program
        .body
        .retain(|s| !matches!(s, Statement::ImportDeclaration(_)));
    let semantic = SemanticBuilder::new().build(&program);
    if !semantic.errors.is_empty() {
        return Err(format!("Invalid module: {:?}", semantic.errors));
    }
    let result = Transformer::new(
        &allocator,
        std::path::Path::new(&manifest.entry),
        &TransformOptions::default(),
    )
    .build_with_scoping(semantic.semantic.into_scoping(), &mut program);
    if !result.errors.is_empty() {
        return Err(format!("Transform error: {:?}", result.errors));
    }
    Ok(Codegen::new().build(&program).code)
}

pub fn validate_ui(tree: &Value) -> Result<(), String> {
    fn walk(v: &Value, depth: usize, nodes: &mut usize) -> Result<(), String> {
        *nodes += 1;
        if depth > 12 || *nodes > 256 {
            return Err("UI tree is too large".into());
        }
        let obj = v.as_object().ok_or("UI node must be an object")?;
        let kind = obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or("Missing UI type")?;
        if !matches!(
            kind,
            "text" | "stack" | "button" | "input" | "table" | "badge" | "progress"
        ) {
            return Err("Unsupported UI type".into());
        }
        if obj.keys().any(|k| {
            ![
                "type",
                "text",
                "children",
                "action",
                "label",
                "value",
                "max",
                "columns",
                "rows",
                "direction",
                "placeholder",
                "tone",
                "color",
            ]
            .contains(&k.as_str())
        }) {
            return Err("Unsupported UI property".into());
        }
        if obj.get("color").is_some_and(|v| {
            !matches!(
                v.as_str(),
                Some("default" | "yellow" | "cyan" | "blue" | "magenta" | "red")
            )
        }) {
            return Err("Unsupported UI color".into());
        }
        if obj
            .get("direction")
            .is_some_and(|v| kind != "stack" || !matches!(v.as_str(), Some("row" | "column")))
        {
            return Err("Stack direction must be row or column".into());
        }
        for key in [
            "text",
            "label",
            "action",
            "placeholder",
            "direction",
            "tone",
        ] {
            if let Some(value) = obj.get(key) {
                if !value.is_string() {
                    return Err(format!("{key} must be a string"));
                }
            }
        }
        if matches!(kind, "text" | "badge") && !obj.get("text").is_some_and(Value::is_string) {
            return Err("Text nodes need text".into());
        }
        if matches!(kind, "button" | "input") && !obj.get("label").is_some_and(Value::is_string) {
            return Err("Controls need a label".into());
        }
        if kind == "input"
            && obj
                .get("value")
                .is_some_and(|v| !v.is_string() && !v.is_number())
        {
            return Err("Input value must be text or number".into());
        }
        if matches!(kind, "button" | "input")
            && !obj
                .get("action")
                .and_then(Value::as_str)
                .is_some_and(valid_id)
        {
            return Err("Controls require a valid action".into());
        }
        if let Some(children) = obj.get("children") {
            for child in children.as_array().ok_or("Children must be an array")? {
                walk(child, depth + 1, nodes)?;
            }
        }
        if kind == "table" {
            if !obj
                .get("columns")
                .and_then(Value::as_array)
                .is_some_and(|c| c.iter().all(Value::is_string))
                || !obj
                    .get("rows")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| {
                        rows.iter().all(|r| {
                            r.as_array().is_some_and(|a| {
                                a.iter()
                                    .all(|c| c.is_string() || c.is_number() || c.is_null())
                            })
                        })
                    })
            {
                return Err("Invalid table".into());
            }
        }
        if kind == "progress"
            && (!obj
                .get("value")
                .and_then(Value::as_f64)
                .is_some_and(|n| n.is_finite() && n >= 0.0)
                || !obj
                    .get("max")
                    .and_then(Value::as_f64)
                    .is_some_and(|n| n.is_finite() && n > 0.0))
        {
            return Err("Invalid progress".into());
        }
        Ok(())
    }
    if tree.to_string().len() > 64 * 1024 {
        return Err("UI exceeds 64 KiB".into());
    }
    walk(tree, 0, &mut 0)
}

pub struct Engine {
    runtime: Runtime,
    context: Context,
    deadline: Arc<Mutex<Duration>>,
}
impl Engine {
    pub fn new(
        compiled: &str,
        state: &Value,
        host: impl Fn(String) -> String + 'static,
    ) -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        runtime.set_memory_limit(64 * 1024 * 1024);
        runtime.set_max_stack_size(512 * 1024);
        let deadline = Arc::new(Mutex::new(cpu_time::ThreadTime::now().as_duration()));
        let d = deadline.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || {
            cpu_time::ThreadTime::now()
                .as_duration()
                .saturating_sub(*d.lock().unwrap())
                >= Duration::from_millis(250)
        })));
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let engine = Self {
            runtime,
            context,
            deadline,
        };
        engine
            .context
            .with(|ctx| -> rquickjs::Result<()> {
                ctx.globals().set("__host", Func::from(host))?;
                ctx.eval::<(), _>(include_str!("sdk.js"))?;
                ctx.eval::<(), _>(format!("__view.state = {};", state))?;
                let module = Module::declare(ctx.clone(), "mod", compiled)?;
                let (module, promise) = module.eval()?;
                promise.finish::<()>()?;
                let register: Function = module.get("register")?;
                let mint: Object = ctx.eval("mint")?;
                let returned: rquickjs::Value = register.call((mint,))?;
                if let Some(promise) = returned.as_promise() {
                    promise.clone().finish::<()>()?;
                }
                Ok(())
            })
            .map_err(|e| engine.error(e))?;
        engine.snapshot()?;
        Ok(engine)
    }
    fn error(&self, error: rquickjs::Error) -> String {
        self.context.with(|ctx| {
            if error.is_exception() {
                format!("Mod exception: {:?}", ctx.catch())
            } else {
                error.to_string()
            }
        })
    }
    pub fn dispatch(&self, kind: &str, payload: &Value) -> Result<Value, String> {
        *self.deadline.lock().unwrap() = cpu_time::ThreadTime::now().as_duration();
        let result = self
            .context
            .with(|ctx| -> rquickjs::Result<String> {
                let promise: Promise =
                    ctx.eval(format!("__dispatch({}, {})", json!(kind), payload))?;
                let value: rquickjs::Value = promise.finish()?;
                ctx.globals().set("__result", value)?;
                ctx.eval("JSON.stringify(__result)")
            })
            .map_err(|e| self.error(e))?;
        let mut result: Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
        result["snapshot"] = self.snapshot()?;
        Ok(result)
    }
    pub fn snapshot(&self) -> Result<Value, String> {
        let text = self
            .context
            .with(|ctx| ctx.eval::<String, _>("JSON.stringify(__view)"))
            .map_err(|e| self.error(e))?;
        if text.len() > 256 * 1024 {
            return Err("Mod state/output exceeds 256 KiB".into());
        }
        let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        validate_snapshot(&value)?;
        let _ = &self.runtime;
        Ok(value)
    }
}
fn validate_snapshot(value: &Value) -> Result<(), String> {
    if value.to_string().len() > 256 * 1024 {
        return Err("Mod state/output exceeds 256 KiB".into());
    }
    for tree in value["ui"]
        .as_object()
        .ok_or("Invalid UI snapshot")?
        .values()
    {
        validate_ui(tree)?;
    }
    Ok(())
}
pub fn preview(manifest: &Value, source: &str) -> Result<Value, String> {
    let compiled = validate_package(manifest, source)?;
    Engine::new(&compiled, &json!({}), |request| {
        let request: Value = serde_json::from_str(&request).unwrap_or_default();
        match request["kind"].as_str() {
            Some("usage") => json!({"value":unavailable_usage()}),
            Some("viewport") => json!({"value":unavailable_viewport()}),
            _ => json!({"error":"Host access is unavailable during preview"}),
        }
        .to_string()
    })?
    .snapshot()
}

/// Newline-delimited IPC. The parent grants capabilities; this process has no OS SDK.
pub fn worker_main() -> Result<(), String> {
    fn read() -> Result<Value, String> {
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        if line.len() > 1024 * 1024 {
            return Err("IPC request too large".into());
        }
        serde_json::from_str(&line).map_err(|e| e.to_string())
    }
    fn send(value: &Value) {
        println!("{}", value);
        let _ = io::stdout().flush();
    }
    let init = read()?;
    let engine = Engine::new(
        init["compiled"].as_str().ok_or("Missing compiled source")?,
        &init["state"],
        |request| {
            let request = serde_json::from_str::<Value>(&request).unwrap_or(Value::Null);
            send(&json!({"host":request}));
            read().unwrap_or_else(|e| json!({"error":e})).to_string()
        },
    );
    let engine = match engine {
        Ok(e) => {
            send(&json!({"ok":e.snapshot()?}));
            e
        }
        Err(e) => {
            send(&json!({"error":e}));
            return Ok(());
        }
    };
    loop {
        let request = match read() {
            Ok(v) => v,
            Err(_) => break,
        };
        let kind = request["kind"].as_str().unwrap_or("");
        if kind == "stop" {
            break;
        }
        match engine.dispatch(kind, &request["payload"]) {
            Ok(v) => send(&json!({"ok":v})),
            Err(e) => {
                send(&json!({"error":e}));
                break;
            }
        }
    }
    Ok(())
}

pub fn pack(manifest: &Value, source: &str, readme: &str) -> Result<Vec<u8>, String> {
    let m: Manifest = serde_json::from_value(manifest.clone()).map_err(|e| e.to_string())?;
    compile(&m, source)?;
    if readme.len() > 64 * 1024 {
        return Err("README exceeds 64 KiB".into());
    }
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, text) in [
        ("manifest.json", serde_json::to_string_pretty(&m).unwrap()),
        (m.entry.as_str(), source.into()),
        ("README.md", readme.into()),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .map_err(|e| e.to_string())?;
        zip.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}
pub fn unpack(bytes: &[u8]) -> Result<Value, String> {
    use std::io::Read;
    if bytes.len() > 1024 * 1024 {
        return Err("Package exceeds 1 MiB".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if !(2..=3).contains(&zip.len()) {
        return Err("A package must contain only manifest, entry and optional README".into());
    }
    let mut files = std::collections::BTreeMap::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_owned();
        if !["manifest.json", "main.ts", "main.js", "README.md"].contains(&name.as_str())
            || file.is_dir()
            || file.unix_mode().is_some_and(|m| m & 0xf000 == 0xa000)
        {
            return Err("Unsafe package entry".into());
        }
        if files.contains_key(&name) {
            return Err("Duplicate package entry".into());
        }
        let mut text = String::new();
        file.take((MAX_SOURCE + 1) as u64)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > MAX_SOURCE {
            return Err("Package entry exceeds limit".into());
        }
        files.insert(name, text);
    }
    let m: Manifest = serde_json::from_str(files.get("manifest.json").ok_or("Missing manifest")?)
        .map_err(|e| e.to_string())?;
    let source = files.get(&m.entry).ok_or("Missing source entry")?;
    if files
        .keys()
        .any(|k| k != "manifest.json" && k != &m.entry && k != "README.md")
    {
        return Err("Unexpected package entry".into());
    }
    compile(&m, source)?;
    let readme = files.get("README.md").cloned().unwrap_or_default();
    if readme.len() > 64 * 1024 {
        return Err("README exceeds limit".into());
    }
    Ok(json!({"manifest":m,"source":source,"readme":readme}))
}
