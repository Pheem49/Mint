use mint_mod_runtime::{MAX_SOURCE, Manifest, compile, content_hash, valid_id};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, LazyLock, Mutex, mpsc},
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub manifest: Manifest,
    pub source: String,
    #[serde(default)]
    pub readme: String,
}
impl Source {
    pub fn hash(&self) -> String {
        content_hash(&self.manifest, &self.source)
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Binding {
    hash: String,
    scope: String,
    chat: String,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    disabled_chats: BTreeSet<String>,
}
#[derive(Default, Serialize, Deserialize)]
struct Catalog {
    bindings: BTreeMap<String, Binding>,
}
struct Instance {
    root: PathBuf,
    chat: String,
    worker: Option<Worker>,
    snapshot: Value,
    error: Option<String>,
    hash: String,
    instance_id: String,
    source: Source,
    reload_error: Option<String>,
    attempted_hash: Option<String>,
}
type InstanceMap = BTreeMap<String, Arc<Mutex<Instance>>>;
static INSTANCES: LazyLock<Mutex<InstanceMap>> = LazyLock::new(|| Mutex::new(BTreeMap::new()));
static WRITES: Mutex<()> = Mutex::new(());

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn canonical(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("Workspace must be a directory".into());
    }
    Ok(root)
}
fn data(root: &Path) -> Result<PathBuf, String> {
    let dir = crate::config_path()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Missing config directory")?
        .join("mods/workspaces")
        .join(digest(&canonical(root)?.to_string_lossy()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
fn global_dir() -> Result<PathBuf, String> {
    let p = crate::config_path()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Missing config directory")?
        .join("mods/global");
    fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p)
}
fn read_json<T: serde::de::DeserializeOwned + Default>(p: &Path) -> Result<T, String> {
    match fs::read(p) {
        Ok(v) => serde_json::from_slice(&v).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.to_string()),
    }
}
pub(super) fn telemetry_path(root: &Path, chat: &str) -> Result<PathBuf, String> {
    Ok(data(root)?.join(format!("context-{}.json", digest(chat))))
}
pub(super) fn atomic_json(p: &Path, value: &impl Serialize) -> Result<(), String> {
    let tmp = p.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(tmp, p).map_err(|e| e.to_string())
}
fn catalog(root: &Path) -> Result<Catalog, String> {
    read_json(&data(root)?.join("catalog.json"))
}
fn source_base(root: &Path, name: &str) -> Result<PathBuf, String> {
    let base = root.join(name);
    match fs::symlink_metadata(&base) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || base.canonicalize().map_err(|e| e.to_string())? != base
            {
                return Err(format!("{name} cannot be a symlink"));
            }
            if !metadata.is_dir() {
                return Err(format!("{name} must be a directory"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    Ok(base)
}
/// Resolve one workspace source: prefer the hidden directory, keep legacy edits
/// in place, and create new IDs in `.mint-mods`. Invalid preferred copies never
/// fall back to a different source with the same ID.
pub fn source_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err("Invalid mod ID".into());
    }
    let root = canonical(root)?;
    let preferred = source_base(&root, ".mint-mods")?.join(id);
    for dir in [preferred.clone(), source_base(&root, "mint-mods")?.join(id)] {
        match fs::symlink_metadata(&dir) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || dir.canonicalize().map_err(|e| e.to_string())? != dir
                {
                    return Err("Mod directories cannot be symlinks".into());
                }
                if !metadata.is_dir() {
                    return Err("Mod source must be a directory".into());
                }
                return Ok(dir);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(preferred)
}
fn limited(path: &Path, max: usize) -> Result<String, String> {
    let real = path.canonicalize().map_err(|e| e.to_string())?;
    if real != path {
        return Err("Mod files cannot be symlinks".into());
    }
    let mut text = String::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((max + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > max {
        return Err("File is too large".into());
    }
    Ok(text)
}
pub fn load_source(root: &Path, id: &str) -> Result<Source, String> {
    let dir = source_path(root, id)?;
    let manifest: Manifest = serde_json::from_str(&limited(&dir.join("manifest.json"), 16 * 1024)?)
        .map_err(|e| e.to_string())?;
    manifest.validate()?;
    if manifest.id != id {
        return Err("Manifest ID must match its directory".into());
    }
    Ok(Source {
        source: limited(&dir.join(&manifest.entry), MAX_SOURCE)?,
        readme: if dir.join("README.md").exists() {
            limited(&dir.join("README.md"), 64 * 1024)?
        } else {
            String::new()
        },
        manifest,
    })
}
/// Use a personal global installation when this workspace has no source copy.
pub fn shareable_source(root: &Path, id: &str) -> Result<Source, String> {
    match load_source(root, id) {
        Ok(source) => Ok(source),
        Err(error) => {
            if source_path(root, id)?.exists() {
                return Err(error);
            }
            let dir = global_dir()?;
            let globals: Catalog = read_json(&dir.join("catalog.json"))?;
            let Some(binding) = globals.bindings.get(id) else {
                return Err(error);
            };
            let source: Source = serde_json::from_slice(
                &fs::read(dir.join(format!("{id}.json"))).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if source.hash() != binding.hash {
                return Err("Global source hash mismatch".into());
            }
            Ok(source)
        }
    }
}
pub fn discover(root: &Path) -> Result<Vec<Value>, String> {
    let root = canonical(root)?;
    let mut ids = BTreeSet::new();
    for name in [".mint-mods", "mint-mods"] {
        let base = source_base(&root, name)?;
        if !base.exists() {
            continue;
        }
        for entry in fs::read_dir(base).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() || kind.is_symlink() {
                ids.insert(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    let mut mods = Vec::new();
    for id in ids {
        match load_source(&root, &id) {
            Ok(s) => mods.push(json!({"id":id,"name":s.manifest.name,"manifest":s.manifest,"hash":s.hash(),"enabled":false})),
            Err(e) => mods.push(json!({"id":id,"name":id,"enabled":false,"error":e})),
        }
    }
    Ok(mods)
}
pub(super) fn key(root: &Path, chat: &str, id: &str) -> Result<String, String> {
    Ok(format!(
        "{}:{}:{}",
        digest(&canonical(root)?.to_string_lossy()),
        digest(chat),
        id
    ))
}
fn state_path(root: &Path, chat: &str, id: &str) -> Result<PathBuf, String> {
    Ok(data(root)?.join(format!("state-{}-{id}.json", digest(chat))))
}
fn revision(root: &Path, id: &str, hash: &str) -> Result<Source, String> {
    if !valid_id(id) || hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid revision".into());
    }
    let p = data(root)?
        .join("revisions")
        .join(id)
        .join(format!("{hash}.json"));
    let source: Source = serde_json::from_slice(&fs::read(p).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if source.hash() != hash {
        return Err("Revision content changed".into());
    }
    Ok(source)
}
fn save_revision(root: &Path, source: &Source) -> Result<(), String> {
    let dir = data(root)?.join("revisions").join(&source.manifest.id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", source.hash()));
    if path.exists() {
        revision(root, &source.manifest.id, &source.hash())?;
        return Ok(());
    }
    atomic_json(&path, source)
}
fn binding_for(root: &Path, chat: &str, id: &str) -> Result<Option<Binding>, String> {
    let catalog = catalog(root)?;
    if let Some(binding) = catalog
        .bindings
        .get(&format!("{id}@{}", digest(chat)))
        .or_else(|| catalog.bindings.get(id))
    {
        if !binding.disabled_chats.contains(chat)
            && (binding.scope == "workspace" || (binding.scope == "chat" && binding.chat == chat))
        {
            return Ok(Some(binding.clone()));
        }
        return Ok(None);
    }
    let globals: Catalog = read_json(&global_dir()?.join("catalog.json"))?;
    Ok(globals
        .bindings
        .get(id)
        .filter(|b| {
            !b.disabled_chats
                .contains(&format!("{}:{chat}", canonical(root).unwrap().display()))
        })
        .cloned())
}
fn all_ids(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut ids: BTreeSet<_> = discover(root)?
        .into_iter()
        .filter_map(|m| m["id"].as_str().map(str::to_owned))
        .collect();
    ids.extend(binding_ids(root)?);
    Ok(ids)
}
// Event delivery must never inspect untrusted, inactive workspace source.
fn binding_ids(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    ids.extend(
        catalog(root)?
            .bindings
            .into_keys()
            .map(|k| k.split('@').next().unwrap().to_owned()),
    );
    let global: Catalog = read_json(&global_dir()?.join("catalog.json"))?;
    ids.extend(global.bindings.into_keys());
    Ok(ids)
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<Result<Value, String>>,
    preview: bool,
    phase: String,
    root: PathBuf,
    chat: String,
    permissions: Vec<String>,
    requests: Vec<Value>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Worker {
    fn start(
        root: &Path,
        chat: &str,
        compiled: &str,
        state: &Value,
        permissions: Vec<String>,
        preview: bool,
    ) -> Result<(Self, Value), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let name = if cfg!(windows) {
            "mint-mod-host.exe"
        } else {
            "mint-mod-host"
        };
        let sibling = exe
            .parent()
            .ok_or("Executable directory missing")?
            .join(name);
        let development = exe.parent().and_then(Path::parent).map(|p| p.join(name));
        let mut command = if sibling.is_file() {
            Command::new(sibling)
        } else if let Some(p) = development.filter(|p| p.is_file()) {
            Command::new(p)
        } else {
            let mut c = Command::new(exe);
            c.arg("--mint-mod-host");
            c
        };
        let mut child = command
            .current_dir(std::env::temp_dir())
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let input = child.stdin.take().ok_or("Worker stdin missing")?;
        let output = child.stdout.take().ok_or("Worker stdout missing")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = String::new();
                let result = reader.by_ref().take(1024 * 1024 + 1).read_line(&mut line);
                match result {
                    Ok(0) => break,
                    Ok(_) if line.len() <= 1024 * 1024 => {
                        let parsed = serde_json::from_str(&line).map_err(|e| e.to_string());
                        if tx.send(parsed).is_err() {
                            break;
                        }
                    }
                    _ => {
                        let _ = tx.send(Err("Worker output exceeds limit".into()));
                        break;
                    }
                }
            }
        });
        let mut worker = Self {
            child,
            input,
            output: rx,
            preview,
            phase: String::new(),
            root: canonical(root)?,
            chat: chat.into(),
            permissions,
            requests: vec![],
        };
        let snapshot = worker.exchange(&json!({"compiled":compiled,"state":state}))?;
        Ok((worker, snapshot))
    }
    fn host(&mut self, request: &Value) -> Value {
        if request["kind"] == "usage" {
            return json!({"value":if self.preview {super::telemetry::UsageTracker::new(&self.root,"",0).snapshot()} else {super::current_usage(&self.root,&self.chat)}});
        }
        if request["kind"] == "viewport" {
            return json!({"value":super::session_snapshot(&self.root,&self.chat).unwrap_or_default()["viewport"]});
        }
        if self.preview {
            return json!({"error":"Host access unavailable during preview"});
        }
        match request["kind"].as_str() {
            Some("read") if self.permissions.iter().any(|p| p == "workspace.read") => {
                let result: Result<String, String> = (|| {
                    let path = request["path"].as_str().ok_or("Missing path")?;
                    let path = self
                        .root
                        .join(path)
                        .canonicalize()
                        .map_err(|e| e.to_string())?;
                    if !path.starts_with(&self.root) {
                        return Err("Path is outside workspace".into());
                    }
                    crate::assert_path_capability(
                        &path,
                        crate::Capability::Read,
                        &crate::load_config().map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    let mut text = String::new();
                    fs::File::open(path)
                        .map_err(|e| e.to_string())?
                        .take(256 * 1024 + 1)
                        .read_to_string(&mut text)
                        .map_err(|e| e.to_string())?;
                    if text.len() > 256 * 1024 {
                        return Err("Read exceeds 256 KiB".into());
                    }
                    Ok(text)
                })();
                match result {
                    Ok(text) => json!({"value":text}),
                    Err(e) => json!({"error":e}),
                }
            }
            Some("tool")
                if self.phase != "pre_tool_use"
                    && self.permissions.iter().any(|p| p == "tools.request") =>
            {
                if self.requests.len() >= 8 {
                    return json!({"error":"Too many pending tool requests"});
                }
                let id = uuid::Uuid::new_v4().to_string();
                self.requests
                    .push(json!({"id":id,"action":request["action"],"input":request["input"]}));
                json!({"value":id})
            }
            _ => json!({"error":"Capability not granted"}),
        }
    }
    fn send(&mut self, value: &Value) -> Result<(), String> {
        writeln!(self.input, "{value}")
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string())
    }
    fn exchange(&mut self, request: &Value) -> Result<Value, String> {
        self.phase = request["kind"].as_str().unwrap_or("register").to_owned();
        self.send(request)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut hosts = 0;
        loop {
            let timeout = deadline.saturating_duration_since(std::time::Instant::now());
            let message = self
                .output
                .recv_timeout(timeout)
                .map_err(|_| "Mod host timed out or exited".to_string())??;
            if let Some(host) = message.get("host") {
                hosts += 1;
                if hosts > 64 {
                    return Err("Too many host calls".into());
                }
                let result = self.host(host);
                self.send(&result)?;
                continue;
            }
            if let Some(error) = message["error"].as_str() {
                return Err(error.into());
            }
            return message
                .get("ok")
                .cloned()
                .ok_or("Invalid worker response".into());
        }
    }
}
pub fn validate(root: &Path, source: &Source) -> Result<Value, String> {
    check_source_size(source)?;
    let compiled = compile(&source.manifest, &source.source)?;
    let (_, snapshot) = Worker::start(root, "", &compiled, &json!({}), vec![], true)?;
    Ok(snapshot)
}
pub fn test_mod(root: &Path, id: &str) -> Result<Value, String> {
    let source = load_source(root, id)?;
    let dir = source_path(root, id)?.join("tests");
    if !dir.is_dir() {
        return Err("No tests directory; add tests/*.test.ts exporting registerTests(test)".into());
    }
    if dir.canonicalize().map_err(|e| e.to_string())? != dir {
        return Err("Mod test directories cannot be symlinks".into());
    }
    let mut files = vec![];
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".test.ts") || name.ends_with(".test.js") {
            files.push((name, limited(&entry.path(), MAX_SOURCE)?));
        }
        if files.len() > 32 {
            return Err("At most 32 mod test files are supported".into());
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    mint_mod_runtime::run_tests(
        &serde_json::to_value(&source.manifest).map_err(|e| e.to_string())?,
        &source.source,
        &files
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str()))
            .collect::<Vec<_>>(),
    )
}
fn check_source_size(source: &Source) -> Result<(), String> {
    if source.source.len() > MAX_SOURCE
        || source.readme.len() > 64 * 1024
        || serde_json::to_vec(&source.manifest)
            .map_err(|e| e.to_string())?
            .len()
            > 16 * 1024
    {
        return Err("Mod source, manifest or README exceeds its size limit".into());
    }
    Ok(())
}
pub fn save_draft(root: &Path, source: &Source) -> Result<PathBuf, String> {
    check_source_size(source)?;
    let dir = data(root)?.join("drafts");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", source.hash()));
    atomic_json(&path, source)?;
    Ok(path)
}
fn ensure(root: &Path, chat: &str, id: &str) -> Result<Option<Arc<Mutex<Instance>>>, String> {
    let Some(base_binding) = binding_for(root, chat, id)? else {
        return Ok(None);
    };
    let mut binding = base_binding.clone();
    if let Some(hash) = super::session::revision(root, chat, id, &binding.hash) {
        binding.hash = hash;
    }
    let k = key(root, chat, id)?;
    let mut instances = INSTANCES.lock().unwrap();
    if let Some(instance) = instances.get(&k).cloned() {
        if instance.lock().unwrap().hash == binding.hash {
            drop(instances);
            try_hot_reload(root, chat, id, &base_binding, &instance);
            return Ok(Some(instance));
        }
        instances.remove(&k);
    }
    let source = revision(root, id, &binding.hash).or_else(|_| {
        let p = global_dir()?.join(format!("{id}.json"));
        serde_json::from_slice::<Source>(&fs::read(p).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    })?;
    if source.hash() != binding.hash {
        return Err("Active source hash mismatch".into());
    }
    let mut state: Value = read_json(&state_path(root, chat, id)?)?;
    if !state.is_object() {
        state = json!({});
    }
    let compiled = compile(&source.manifest, &source.source)?;
    let started = Worker::start(root, chat, &compiled, &state, binding.permissions, false);
    let (worker, snapshot, error) = match started {
        Ok((w, s)) => (Some(w), s, None),
        Err(e) => (
            None,
            json!({"ui":{},"state":{},"logs":[],"commands":[]}),
            Some(e),
        ),
    };
    let instance = Arc::new(Mutex::new(Instance {
        root: canonical(root)?,
        chat: chat.into(),
        worker,
        snapshot,
        error,
        hash: binding.hash,
        source,
        reload_error: None,
        attempted_hash: None,
        instance_id: uuid::Uuid::new_v4().to_string(),
    }));
    let _ = dispatch_one(
        root,
        chat,
        id,
        &instance,
        "session_start",
        &json!({"chatId":chat,"workspace":canonical(root)?}),
    );
    instances.insert(k, instance.clone());
    drop(instances);
    Ok(Some(instance))
}
fn try_hot_reload(
    root: &Path,
    chat: &str,
    id: &str,
    base: &Binding,
    instance: &Arc<Mutex<Instance>>,
) {
    if base.scope == "global" || !super::session::hot_reload(root, chat) {
        return;
    }
    let _guard = WRITES.lock().unwrap();
    let mut current = instance.lock().unwrap();
    let source = match load_source(root, id) {
        Ok(s) => s,
        Err(e) => {
            current.reload_error = Some(e);
            return;
        }
    };
    let hash = source.hash();
    if hash == current.hash {
        current.reload_error = None;
        current.attempted_hash = None;
        return;
    }
    if current.attempted_hash.as_deref() == Some(&hash) {
        return;
    }
    current.attempted_hash = Some(hash.clone());
    if !base.permissions.is_empty() || !source.manifest.permissions.is_empty() {
        current.reload_error=Some("Source changed; review and enable this revision before reloading a mod with permissions".into());
        return;
    }
    let candidate = (|| -> Result<(Worker, Value), String> {
        let compiled = compile(&source.manifest, &source.source)?;
        let (mut worker, _) = Worker::start(
            root,
            chat,
            &compiled,
            &current.snapshot["state"],
            vec![],
            false,
        )?;
        let response=worker.exchange(&json!({"kind":"session_start","payload":{"chatId":chat,"workspace":canonical(root)?,"reloaded":true}}))?;
        let snapshot = response
            .get("snapshot")
            .cloned()
            .ok_or("Reload snapshot missing")?;
        // Confirm the backing activation still grants this session access.
        if binding_for(root, chat, id)?.is_none_or(|b| b.hash != base.hash) {
            return Err("Activation changed during reload".into());
        }
        save_revision(root, &source)?;
        atomic_json(&state_path(root, chat, id)?, &snapshot["state"])?;
        super::session::set_revision(root, chat, id, &base.hash, &hash)?;
        Ok((worker, snapshot))
    })();
    match candidate {
        Ok((worker, snapshot)) => {
            current.worker = Some(worker);
            current.snapshot = snapshot;
            current.error = None;
            current.reload_error = None;
            current.hash = hash;
            current.source = source;
            current.instance_id = uuid::Uuid::new_v4().to_string();
        }
        Err(e) => current.reload_error = Some(e),
    }
}
fn dispatch_one(
    root: &Path,
    chat: &str,
    id: &str,
    instance: &Arc<Mutex<Instance>>,
    kind: &str,
    payload: &Value,
) -> Result<Value, String> {
    let mut instance = instance.lock().unwrap();
    if let Some(error) = &instance.error {
        return Err(error.clone());
    }
    let worker = instance.worker.as_mut().ok_or("Mod is stopped")?;
    match worker.exchange(&json!({"kind":kind,"payload":payload})) {
        Ok(result) => {
            let snapshot = result.get("snapshot").cloned().ok_or("Snapshot missing")?;
            atomic_json(&state_path(root, chat, id)?, &snapshot["state"])?;
            instance.snapshot = snapshot;
            Ok(result)
        }
        Err(error) => {
            instance.error = Some(error.clone());
            instance.worker.take();
            Err(error)
        }
    }
}
pub fn event(
    root: &Path,
    chat: &str,
    kind: &str,
    payload: Value,
    origin: Option<&str>,
) -> Result<(), String> {
    if matches!(kind, "usage_update" | "context_compacted" | "turn_end") {
        let _ = super::session::store_usage(root, chat, &payload);
    }
    for id in binding_ids(root)? {
        if origin == Some(id.as_str()) {
            continue;
        }
        let instance = match ensure(root, chat, &id) {
            Ok(Some(i)) => i,
            Ok(None) => continue,
            Err(e) => {
                if kind == "pre_tool_use" {
                    return Err(format!("{id}: {e}"));
                }
                continue;
            }
        };
        let mut payload = payload.clone();
        payload["chatId"] = json!(chat);
        payload["workspace"] = json!(canonical(root)?);
        match dispatch_one(root, chat, &id, &instance, kind, &payload) {
            Ok(result) => {
                if kind == "pre_tool_use" {
                    if let Some(reason) = result["block"].as_str() {
                        return Err(format!("Blocked by mod {id}: {reason}"));
                    }
                }
            }
            Err(e) => {
                if kind == "pre_tool_use" {
                    return Err(format!(
                        "Mod {id} failed: {e}. Fix or disable it before continuing."
                    ));
                }
            }
        }
    }
    Ok(())
}
pub fn list(root: &Path, chat: &str) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    for id in all_ids(root)? {
        let source = shareable_source(root, &id);
        let mut item = match &source {
            Ok(s) => {
                json!({"id":id,"name":s.manifest.name,"manifest":s.manifest,"hash":s.hash(),"source":s.source,"readme":s.readme})
            }
            Err(e) => json!({"id":id,"name":id,"sourceError":e}),
        };
        item["enabled"] = json!(false);
        match ensure(root, chat, &id) {
            Ok(Some(instance)) => {
                let i = instance.lock().unwrap();
                item["enabled"] = json!(true);
                item["activeHash"] = json!(i.hash);
                item["instanceId"] = json!(i.instance_id);
                item["snapshot"] = i.snapshot.clone();
                item["error"] = json!(i.error);
                item["reloadError"] = json!(i.reload_error);
                item["changed"] = json!(source.as_ref().is_ok_and(|s| s.hash() != i.hash));
                if source.is_err() {
                    item["source"] = json!(i.source.source);
                    item["manifest"] = json!(i.source.manifest);
                }
            }
            Ok(None) => {}
            Err(e) => item["error"] = json!(e),
        }
        item["revisions"] = json!(revisions(root, &id)?);
        result.push(item);
    }
    Ok(result)
}
pub fn enable(root: &Path, chat: &str, id: &str, scope: &str, grant: bool) -> Result<(), String> {
    enable_reviewed(root, chat, id, scope, grant, None)
}
pub fn enable_reviewed(
    root: &Path,
    chat: &str,
    id: &str,
    scope: &str,
    grant: bool,
    expected: Option<&str>,
) -> Result<(), String> {
    if !matches!(scope, "chat" | "workspace" | "global") {
        return Err("Invalid activation scope".into());
    }
    let source = shareable_source(root, id)?;
    if expected.is_some_and(|hash| hash != source.hash()) {
        return Err(
            "Mod source changed after review; inspect and approve the current revision".into(),
        );
    }
    validate(root, &source)?;
    if !source.manifest.permissions.is_empty() && !grant {
        return Err(format!(
            "Approval required for permissions: {}",
            source.manifest.permissions.join(", ")
        ));
    }
    let _guard = WRITES.lock().unwrap();
    save_revision(root, &source)?;
    let binding = Binding {
        hash: source.hash(),
        scope: scope.into(),
        chat: chat.into(),
        permissions: source.manifest.permissions.clone(),
        disabled_chats: BTreeSet::new(),
    };
    if scope == "global" {
        let dir = global_dir()?;
        atomic_json(&dir.join(format!("{id}.json")), &source)?;
        let mut c: Catalog = read_json(&dir.join("catalog.json"))?;
        c.bindings.insert(id.into(), binding);
        atomic_json(&dir.join("catalog.json"), &c)?;
        let mut local = catalog(root)?;
        local.bindings.remove(id);
        local.bindings.remove(&format!("{id}@{}", digest(chat)));
        atomic_json(&data(root)?.join("catalog.json"), &local)?;
    } else {
        let mut c = catalog(root)?;
        if scope == "workspace" {
            c.bindings.remove(&format!("{id}@{}", digest(chat)));
        }
        c.bindings.insert(
            if scope == "chat" {
                format!("{id}@{}", digest(chat))
            } else {
                id.into()
            },
            binding,
        );
        atomic_json(&data(root)?.join("catalog.json"), &c)?;
    }
    stop_instances(root, id)?;
    drop(_guard);
    ensure(root, chat, id)?;
    Ok(())
}
fn stop_instances(root: &Path, id: &str) -> Result<(), String> {
    let prefix = format!("{}:", digest(&canonical(root)?.to_string_lossy()));
    let mut instances = INSTANCES.lock().unwrap();
    instances.retain(|k, _| !(k.starts_with(&prefix) && k.ends_with(&format!(":{id}"))));
    Ok(())
}
pub fn disable(root: &Path, chat: &str, id: &str) -> Result<(), String> {
    let _guard = WRITES.lock().unwrap();
    let mut c = catalog(root)?;
    let binding_key = format!("{id}@{}", digest(chat));
    c.bindings.insert(
        binding_key,
        Binding {
            scope: "chat".into(),
            chat: chat.into(),
            disabled_chats: BTreeSet::from([chat.into()]),
            ..Default::default()
        },
    );
    atomic_json(&data(root)?.join("catalog.json"), &c)?;
    if let Some(instance) = INSTANCES.lock().unwrap().remove(&key(root, chat, id)?) {
        let _ = dispatch_one(
            root,
            chat,
            id,
            &instance,
            "session_end",
            &json!({"chatId":chat}),
        );
    }
    Ok(())
}
pub fn close_chat(root: &Path, chat: &str) {
    super::session::clear(root, chat);
    if let Ok(ids) = binding_ids(root) {
        for id in ids {
            if let Ok(k) = key(root, chat, &id) {
                if let Some(instance) = INSTANCES.lock().unwrap().remove(&k) {
                    let _ = dispatch_one(
                        root,
                        chat,
                        &id,
                        &instance,
                        "session_end",
                        &json!({"chatId":chat}),
                    );
                }
            }
        }
    }
}
/// Stop all instances for a cancelled/deleted chat, including abort-only paths.
pub fn shutdown_chat(chat: &str) {
    let marker = format!(":{}:", digest(chat));
    super::session::clear_matching(&marker);
    let removed: Vec<_> = {
        let mut instances = INSTANCES.lock().unwrap();
        let keys: Vec<_> = instances
            .keys()
            .filter(|k| k.contains(&marker))
            .cloned()
            .collect();
        keys.into_iter()
            .filter_map(|k| instances.remove(&k))
            .collect()
    };
    for instance in removed {
        let (root, chat, id) = {
            let i = instance.lock().unwrap();
            (i.root.clone(), i.chat.clone(), i.source.manifest.id.clone())
        };
        super::session::clear(&root, &chat);
        let _ = dispatch_one(
            &root,
            &chat,
            &id,
            &instance,
            "session_end",
            &json!({"chatId":chat}),
        );
    }
}
pub fn action(
    root: &Path,
    chat: &str,
    id: &str,
    instance_id: &str,
    action: &str,
    value: Value,
) -> Result<Value, String> {
    let instance = ensure(root, chat, id)?.ok_or("Mod is not enabled")?;
    if instance.lock().unwrap().instance_id != instance_id {
        return Err("Stale mod action".into());
    }
    dispatch_one(
        root,
        chat,
        id,
        &instance,
        "action",
        &json!({"action":action,"value":value}),
    )
}
pub fn command(root: &Path, chat: &str, id: &str, name: &str, args: &str) -> Result<Value, String> {
    let instance = ensure(root, chat, id)?.ok_or("Mod is not enabled")?;
    dispatch_one(
        root,
        chat,
        id,
        &instance,
        "action",
        &json!({"action":name,"value":args}),
    )
}
pub fn take_requests(root: &Path, chat: &str) -> Result<Vec<Value>, String> {
    let mut requests = Vec::new();
    for id in binding_ids(root)? {
        if let Some(instance) = ensure(root, chat, &id)? {
            let mut instance = instance.lock().unwrap();
            let hash = instance.hash.clone();
            if let Some(worker) = instance.worker.as_mut() {
                for mut r in worker.requests.drain(..) {
                    r["modId"] = json!(id);
                    r["hash"] = json!(hash);
                    requests.push(r);
                }
            }
        }
    }
    Ok(requests)
}
pub fn revisions(root: &Path, id: &str) -> Result<Vec<String>, String> {
    if !valid_id(id) {
        return Ok(vec![]);
    }
    let path = data(root)?.join("revisions").join(id);
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut hashes = vec![];
    for f in fs::read_dir(path).map_err(|e| e.to_string())? {
        let f = f.map_err(|e| e.to_string())?;
        if let Some(hash) = f.path().file_stem().and_then(|s| s.to_str()) {
            hashes.push(hash.to_owned());
        }
    }
    hashes.sort();
    Ok(hashes)
}
pub fn publish(root: &Path, source: &Source, expected: Option<&str>) -> Result<String, String> {
    validate(root, source)?;
    let _guard = WRITES.lock().unwrap();
    let dir = source_path(root, &source.manifest.id)?;
    if dir.exists() {
        let old = load_source(root, &source.manifest.id)?;
        if expected != Some(old.hash().as_str()) {
            return Err("Mod changed on disk; inspect current source before replacing".into());
        }
        save_revision(root, &old)?;
    } else if expected.is_some() {
        return Err("Mod was removed on disk".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    source_path(root, &source.manifest.id)?;
    for filename in ["manifest.json", source.manifest.entry.as_str(), "README.md"] {
        let p = dir.join(filename);
        if p.exists() && p.canonicalize().map_err(|e| e.to_string())? != p {
            return Err("Cannot overwrite symlinks".into());
        }
    }
    save_revision(root, source)?;
    atomic_json(&dir.join("manifest.json"), &source.manifest)?;
    fs::write(dir.join(&source.manifest.entry), &source.source).map_err(|e| e.to_string())?;
    fs::write(dir.join("README.md"), &source.readme).map_err(|e| e.to_string())?;
    Ok(source.hash())
}
pub fn rollback(root: &Path, id: &str, hash: &str, expected: &str) -> Result<String, String> {
    let source = revision(root, id, hash)?;
    publish(root, &source, Some(expected))
}

pub fn complete_request(
    root: &Path,
    chat: &str,
    id: &str,
    hash: &str,
    payload: Value,
) -> Result<(), String> {
    if let Some(instance) = ensure(root, chat, id)? {
        if instance.lock().unwrap().hash == hash {
            let _ = dispatch_one(root, chat, id, &instance, "tool_result", &payload);
        }
    }
    Ok(())
}
pub fn pending_requests(root: &Path, chat: &str) -> Result<Vec<Value>, String> {
    let mut result = vec![];
    for id in binding_ids(root)? {
        if let Some(i) = ensure(root, chat, &id)? {
            let i = i.lock().unwrap();
            if let Some(worker) = &i.worker {
                for r in &worker.requests {
                    let mut r = r.clone();
                    r["modId"] = json!(id);
                    r["hash"] = json!(i.hash);
                    result.push(r);
                }
            }
        }
    }
    Ok(result)
}

pub fn take_selected_requests(
    root: &Path,
    chat: &str,
    selected: &[String],
) -> Result<Vec<Value>, String> {
    let mut result = vec![];
    for id in binding_ids(root)? {
        if let Some(instance) = ensure(root, chat, &id)? {
            let mut i = instance.lock().unwrap();
            let hash = i.hash.clone();
            if let Some(worker) = i.worker.as_mut() {
                let mut remaining = vec![];
                for mut r in worker.requests.drain(..) {
                    if selected.iter().any(|s| r["id"].as_str() == Some(s)) {
                        r["modId"] = json!(id);
                        r["hash"] = json!(hash);
                        result.push(r);
                    } else {
                        remaining.push(r);
                    }
                }
                worker.requests = remaining;
            }
        }
    }
    Ok(result)
}
static POLICIES: LazyLock<Mutex<BTreeMap<String, (bool, Option<String>)>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
pub fn set_turn_policy(root: &Path, chat: &str, plan: bool, pin: Option<&str>) {
    if let Ok(k) = key(root, chat, "policy") {
        POLICIES
            .lock()
            .unwrap()
            .insert(k, (plan, pin.map(str::to_owned)));
    }
}
pub fn turn_policy(root: &Path, chat: &str) -> (bool, Option<String>) {
    key(root, chat, "policy")
        .ok()
        .and_then(|k| POLICIES.lock().unwrap().get(&k).cloned())
        .unwrap_or_default()
}
