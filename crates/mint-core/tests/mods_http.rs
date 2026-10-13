use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Server {
    child: Child,
    address: SocketAddr,
    root: PathBuf,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Server {
    fn start() -> Self {
        Self::start_in("mint-mods")
    }
    fn start_in(directory: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let root = std::env::temp_dir().join(format!("mint-mod-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join(directory).join("checklist");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.json"),r#"{"id":"checklist","name":"Checklist","version":"1","sdkVersion":1,"entry":"main.ts","permissions":[]}"#).unwrap();
        std::fs::write(dir.join("main.ts"),r#"export function register(mint){function draw(){mint.ui.set('side_panel',{type:'text',text:mint.state.get('task')||'Empty'});}mint.commands.register('add',v=>{mint.state.set('task',v);draw();});draw();}"#).unwrap();
        let config = mint_core::MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            ..Default::default()
        };
        std::fs::write(
            root.join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "mods_http_worker", "--nocapture"])
            .env("MINT_MOD_HTTP_ADDRESS", address.to_string())
            .env("MINT_CONFIG_PATH", root.join("config.json"))
            .current_dir(&root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut server = Self {
            child,
            address,
            root,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(address).is_err() {
            assert!(server.child.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        server
    }
    fn request(&self, mut op: Value) -> (String, Value) {
        if op.get("workspacePath").is_none() {
            op["workspacePath"] = json!(self.root);
        }
        if op.get("chatId").is_none() {
            op["chatId"] = json!("a");
        }
        self.post("/api/mods", op)
    }
    fn post(&self, route: &str, op: Value) -> (String, Value) {
        let body = op.to_string();
        let mut socket = TcpStream::connect(self.address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        write!(socket,"POST {route} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",body.len(),body).unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        let (header, body) = response.split_once("\r\n\r\n").unwrap();
        (
            header.lines().next().unwrap().into(),
            serde_json::from_str(body).unwrap(),
        )
    }
}
#[test]
fn mods_http_worker() {
    let Ok(address) = std::env::var("MINT_MOD_HTTP_ADDRESS") else {
        return;
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        mint_core::api_server::start_api_server_on(address.parse().unwrap())
            .await
            .unwrap();
    });
}
#[test]
fn http_mods_preserve_chat_identity_and_reject_stale_actions() {
    let server = Server::start();
    let (status, list) = server.request(json!({"action":"list"}));
    assert!(status.contains("200"));
    assert_eq!(list["mods"][0]["enabled"], false);
    let (status, list) = server.request(json!({"action":"enable","id":"checklist"}));
    assert!(status.contains("200"));
    let instance = list["mods"][0]["instanceId"].as_str().unwrap();
    let(status,_)=server.request(json!({"action":"action","id":"checklist","instanceId":instance,"command":"add","value":"Test HTTP"}));
    assert!(status.contains("200"));
    let (_, list) = server.request(json!({"action":"list"}));
    assert_eq!(
        list["mods"][0]["snapshot"]["ui"]["side_panel"]["text"],
        "Test HTTP"
    );
    let (_, other) = server.request(json!({"action":"list","chatId":"b"}));
    assert_eq!(other["mods"][0]["enabled"], false);
    let(status,_)=server.request(json!({"action":"action","id":"checklist","instanceId":"stale","command":"add","value":"Wrong"}));
    assert!(status.contains("400"));
    let hash = list["mods"][0]["hash"].as_str().unwrap();
    let (_, exported) = server.post("/api/slash", json!({"input":"/mods export checklist 'checklist exported.mintmod'","cwd":server.root,"chat_id":"a"}));
    assert!(exported["markdown"].as_str().unwrap().contains("Exported"));
    let import_root = server.root.join("import-target");
    std::fs::create_dir(&import_root).unwrap();
    let (_, imported) = server.post("/api/slash", json!({"input":"/mods import '../checklist exported.mintmod'","cwd":import_root,"chat_id":"a"}));
    assert!(imported["markdown"].as_str().unwrap().contains("Imported"));
    assert!(import_root.join(".mint-mods/checklist/main.ts").is_file());
    assert!(!import_root.join("mint-mods").exists());
    let (_, discovered) = server.request(json!({"action":"list","workspacePath":import_root}));
    assert_eq!(discovered["mods"][0]["enabled"], false);
    let (status, _) =
        server.request(json!({"action":"enable","id":"checklist","expectedHash":"stale"}));
    assert!(status.contains("400"));
    let (status, _) = server.request(
        json!({"action":"enable","id":"checklist","scope":"workspace","expectedHash":hash}),
    );
    assert!(status.contains("200"));
    let (_, other) = server.request(json!({"action":"list","chatId":"b"}));
    assert_eq!(other["mods"][0]["enabled"], true);
    assert_eq!(
        other["mods"][0]["snapshot"]["ui"]["side_panel"]["text"],
        "Empty"
    );
    let (status, _) = server
        .request(json!({"action":"enable","id":"checklist","scope":"global","expectedHash":hash}));
    assert!(status.contains("200"));
    let other_workspace = server.root.join("another-workspace");
    std::fs::create_dir(&other_workspace).unwrap();
    let (_, globals) =
        server.request(json!({"action":"list","chatId":"c","workspacePath":other_workspace}));
    assert_eq!(globals["mods"][0]["enabled"], true);
    let (status, _) = server.request(
        json!({"action":"export","id":"checklist","chatId":"c","workspacePath":other_workspace}),
    );
    assert!(status.contains("200"));
    let (_, package) = server.request(json!({"action":"export","id":"checklist"}));
    assert_eq!(package["filename"], "checklist.mintmod");
    let (status, _) = server.request(json!({"action":"list","chatId":""}));
    assert!(status.contains("400"));
}

#[test]
fn http_session_controls_viewport_and_mod_tests_share_the_core() {
    let server = Server::start();
    let (status, value) = server.request(json!({"action":"hot_reload","value":true}));
    assert!(status.contains("200"), "{status}: {value}");
    assert_eq!(value["session"]["hotReload"], true);
    let (_, other) = server.request(json!({"action":"list","chatId":"b"}));
    assert_eq!(other["session"]["hotReload"], false);
    let (status,value)=server.request(json!({"action":"viewport","viewport":{"surface":"web","columns":64,"widthPx":512,"heightPx":128}}));
    assert!(status.contains("200"), "{value}");
    assert_eq!(value["session"]["viewport"]["columns"], 64);
    let (status, _) =
        server.request(json!({"action":"viewport","viewport":{"surface":"web","columns":0}}));
    assert!(status.contains("400"));
    let dir = server.root.join("mint-mods/checklist/tests");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("checklist.test.ts"),"export function registerTests(test){test('adds task',async({command,ui,equal})=>{await command('add','hello');equal(ui('side_panel').text,'hello');});}").unwrap();
    let (status, value) = server.request(json!({"action":"test","id":"checklist"}));
    assert!(status.contains("200"), "{value}");
    assert_eq!(value["testReport"]["passed"], 1);
    let (_, value) = server.request(json!({"action":"close"}));
    assert_eq!(value["session"]["hotReload"], false);
}

#[test]
fn hidden_mods_use_the_shared_desktop_web_operations_for_import_and_tests() {
    let server = Server::start_in(".mint-mods");
    let (status, listed) = server.request(json!({"action":"list"}));
    assert!(status.contains("200"), "{listed}");
    assert_eq!(listed["mods"][0]["id"], "checklist");
    let (_, exported) = server.request(json!({"action":"export","id":"checklist"}));
    let target = server.root.join("new-workspace");
    std::fs::create_dir(&target).unwrap();
    let (status, imported) = server.request(json!({
        "action":"import","workspacePath":target,"package":exported["package"]
    }));
    assert!(status.contains("200"), "{imported}");
    assert!(target.join(".mint-mods/checklist/main.ts").is_file());
    assert!(!target.join("mint-mods").exists());
    assert_eq!(imported["mods"][0]["enabled"], false);
    let tests = target.join(".mint-mods/checklist/tests");
    std::fs::create_dir(&tests).unwrap();
    std::fs::write(tests.join("checklist.test.ts"),
        "export function registerTests(test){test('adds task',async({command,ui,equal})=>{await command('add','hidden');equal(ui('side_panel').text,'hidden');});}"
    ).unwrap();
    let (status, report) =
        server.request(json!({"action":"test","workspacePath":target,"id":"checklist"}));
    assert!(status.contains("200"), "{report}");
    assert_eq!(report["testReport"]["failed"], 0);
    assert_eq!(report["testReport"]["passed"], 1);
}
