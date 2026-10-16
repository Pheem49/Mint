//! Exercise the real authenticated Web intake and catalog without contacting an AI provider.
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
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let root = std::env::temp_dir().join(format!("mint-library-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = mint_core::MintConfig::default();
        config
            .extra
            .insert("apiAuthToken".into(), json!("library-test-token"));
        std::fs::write(
            root.join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        drop(listener);
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "library_http_worker", "--nocapture"])
            .env("MINT_LIBRARY_TEST_ADDRESS", address.to_string())
            .env("MINT_CONFIG_PATH", root.join("config.json"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
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
    fn request(
        &self,
        method: &str,
        path: &str,
        body: &str,
        authenticated: bool,
    ) -> (String, String) {
        let mut socket = TcpStream::connect(self.address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let auth = if authenticated {
            "Authorization: Bearer library-test-token\r\n"
        } else {
            ""
        };
        write!(socket,"{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{auth}\r\n{body}",body.len()).unwrap();
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).unwrap();
        let response = String::from_utf8(bytes).unwrap();
        let (header, body) = response.split_once("\r\n\r\n").unwrap();
        (header.lines().next().unwrap().into(), body.into())
    }
    fn json(&self, method: &str, path: &str, body: &str) -> (String, Value) {
        let (status, body) = self.request(method, path, body, true);
        (status, serde_json::from_str(&body).unwrap())
    }
}
#[test]
fn library_http_worker() {
    let Ok(address) = std::env::var("MINT_LIBRARY_TEST_ADDRESS") else {
        return;
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        mint_core::api_server::start_api_server_on(address.parse().unwrap())
            .await
            .unwrap()
    });
}
#[test]
fn library_http_intake_retry_trash_restore_and_history_deletion_preserve_original() {
    let server = Server::start();
    assert!(
        server
            .request("GET", "/api/library", "", false)
            .0
            .contains("401")
    );
    let body=json!({"message":"/chat Read this","chatId":"A","documentAttachment":{"filename":"failed-extraction.pdf","dataUri":"data:application/pdf;base64,aGVsbG8="},"attachmentMetadata":{"requestId":"same-submission","imageNames":[],"videoNames":[]}}).to_string();
    for _ in 0..2 {
        assert!(
            server
                .request("POST", "/api/chat-stream", &body, true)
                .0
                .contains("400")
        );
    }
    let (status, page) = server.json("GET", "/api/library?kind=file&chatId=A", "");
    assert!(status.contains("200"));
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let item = &page["items"][0];
    let id = item["id"].as_str().unwrap();
    assert_eq!(item["name"], "failed-extraction.pdf");
    assert_eq!(item["size"], 5);
    assert!(item.get("path").is_none());
    let content = format!("/api/library/{id}/content");
    assert_eq!(server.request("GET", &content, "", true).1, "hello");
    let (_, sources) = server.json("GET", "/api/conversation-sources?chatId=A", "");
    assert_eq!(sources["attachments"][0]["libraryItemId"], id);
    assert!(sources["attachments"][0]["interactionId"].is_null());
    assert!(
        server
            .json("POST", &format!("/api/library/{id}/trash"), "")
            .0
            .contains("200")
    );
    assert!(server.request("GET", &content, "", true).0.contains("400"));
    let (_, active) = server.json("GET", "/api/library", "");
    assert!(active["items"].as_array().unwrap().is_empty());
    let (_, trash) = server.json("GET", "/api/library?trashed=true", "");
    assert_eq!(trash["items"][0]["id"], id);
    let (_, sources) = server.json("GET", "/api/conversation-sources?chatId=A", "");
    assert_eq!(sources["attachments"][0]["trashed"], true);
    assert!(
        server
            .json("POST", &format!("/api/library/{id}/restore"), "")
            .0
            .contains("200")
    );
    assert_eq!(server.request("GET", &content, "", true).1, "hello");
    assert!(
        server
            .json("POST", "/api/interactions/clear?chatId=A", "")
            .0
            .contains("200")
    );
    let (_, sources) = server.json("GET", "/api/conversation-sources?chatId=A", "");
    assert!(sources["attachments"].as_array().unwrap().is_empty());
    let (_, page) = server.json("GET", "/api/library", "");
    assert_eq!(page["items"][0]["id"], id);
    assert!(
        page["items"][0]["references"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.request("GET", &content, "", true).1, "hello");
    assert!(
        server
            .request("DELETE", &format!("/api/library/{id}"), "", false)
            .0
            .contains("401")
    );
    assert!(
        server
            .request("DELETE", &format!("/api/library/{id}"), "", true)
            .0
            .contains("400")
    );
    server.json("POST", &format!("/api/library/{id}/trash"), "");
    assert!(
        server
            .request("DELETE", &format!("/api/library/{id}"), "", true)
            .0
            .contains("200")
    );
    assert!(
        server
            .request("GET", &format!("/api/library/{id}"), "", true)
            .0
            .contains("400")
    );
    let (_, trash) = server.json("GET", "/api/library?trashed=true", "");
    assert!(trash["items"].as_array().unwrap().is_empty());
    // The child process uses an isolated XDG_DATA_HOME, so this is the actual
    // desktop Trash implementation without touching the user's Trash.
    #[cfg(target_os = "linux")]
    {
        let files = server.root.join("data/Trash/files");
        assert!(
            std::fs::read_dir(files)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| std::fs::read(entry.path()).is_ok_and(|bytes| bytes == b"hello"))
        );
    }
}
