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
        Self::configured(|_, _, _| {})
    }

    fn configured(
        setup: impl FnOnce(&mut mint_core::MintConfig, SocketAddr, &std::path::Path),
    ) -> Self {
        Self::configured_on("127.0.0.1", setup)
    }

    fn configured_on(
        host: &str,
        setup: impl FnOnce(&mut mint_core::MintConfig, SocketAddr, &std::path::Path),
    ) -> Self {
        let reservation = TcpListener::bind((host, 0)).unwrap();
        let address = reservation.local_addr().unwrap();
        let root = std::env::temp_dir().join(format!("mint-catalog-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = mint_core::MintConfig::default();
        setup(&mut config, address, &root);
        std::fs::write(
            root.join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        drop(reservation);
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "model_catalog_http_worker", "--nocapture"])
            .env("MINT_CATALOG_TEST_ADDRESS", address.to_string())
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
        let deadline = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect(address).is_err() {
            assert!(
                server.child.try_wait().unwrap().is_none(),
                "HTTP worker exited during startup"
            );
            assert!(Instant::now() < deadline, "HTTP server did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        server
    }

    fn request(&self, method: &str, path: &str, headers: &str, body: &[u8]) -> (String, String) {
        Self::request_at(self.address, method, path, headers, body)
    }

    fn request_at(
        address: SocketAddr,
        method: &str,
        path: &str,
        headers: &str,
        body: &[u8],
    ) -> (String, String) {
        let mut socket = TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(socket, "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n", body.len()).unwrap();
        socket.write_all(body).unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").expect("HTTP response");
        (headers.lines().next().unwrap().to_owned(), body.to_owned())
    }

    fn get_models(&self, query: &str) -> Vec<String> {
        let (status, body) = self.request("GET", query, "", &[]);
        assert_eq!(status, "HTTP/1.1 200 OK", "{query}: {body}");
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        serde_json::from_value(body["models"].clone()).unwrap()
    }
}

#[test]
fn model_catalog_http_worker() {
    let Ok(address) = std::env::var("MINT_CATALOG_TEST_ADDRESS") else {
        return;
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        mint_core::api_server::start_api_server_on(address.parse().unwrap())
            .await
            .unwrap();
    });
}

#[test]
fn media_model_routes_return_presets_without_credentials() {
    let server = Server::start();
    for (query, expected_first) in [
        (
            "/api/image-models?provider=%20OpEnAi%20&apiKey=%20",
            "gpt-image-1",
        ),
        (
            "/api/video-models?provider=%20GOOGLE%20&apiKey=%20",
            "veo-3.1-generate-preview",
        ),
        (
            "/api/live-models?apiKey=%20",
            "gemini-2.5-flash-native-audio-preview-12-2025",
        ),
    ] {
        let models = server.get_models(query);
        assert_eq!(
            models.first().map(String::as_str),
            Some(expected_first),
            "{query}"
        );
    }
}

#[test]
fn media_model_routes_keep_presets_when_saved_config_is_unreadable() {
    let server = Server::start();
    std::fs::write(server.root.join("config.json"), "malformed config").unwrap();
    for (query, expected_first) in [
        ("/api/image-models?provider=openai", "gpt-image-1"),
        ("/api/video-models?provider=veo", "veo-3.1-generate-preview"),
        (
            "/api/live-models",
            "gemini-2.5-flash-native-audio-preview-12-2025",
        ),
    ] {
        let models = server.get_models(query);
        assert_eq!(
            models.first().map(String::as_str),
            Some(expected_first),
            "{query}"
        );
    }
}

#[test]
fn core_migration_http_skill_inventory_uses_selected_workspace_and_location_metadata() {
    let server = Server::configured(|_, _, root| {
        for (workspace, text) in [
            ("one", "first workspace"),
            ("two + %2F", "selected workspace"),
        ] {
            let directory = root.join(workspace).join("skills");
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("chosen.md"), text).unwrap();
        }
    });
    let workspace = server.root.join("two + %2F");
    let mut url = reqwest::Url::parse("http://localhost/api/learned-skills").unwrap();
    url.query_pairs_mut()
        .append_pair("workspacePath", &workspace.to_string_lossy());
    let path = format!("{}?{}", url.path(), url.query().unwrap());
    let (status, body) = server.request("GET", &path, "", &[]);
    assert_eq!(status, "HTTP/1.1 200 OK");
    let skills: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    let selected = skills
        .iter()
        .find(|s| s["name"] == "chosen")
        .unwrap_or_else(|| panic!("skill from selected workspace; response: {body}"));
    assert_eq!(selected["content"], "selected workspace");
    assert_eq!(selected["location"], "workspace");
    assert_eq!(selected["is_workspace"], true);
    assert_eq!(selected["updatedAt"], "");
}

fn webhook_server() -> Server {
    Server::configured(|config, address, _| {
        config.extra.extend([
            ("apiAuthToken".into(), serde_json::json!("api-secret")),
            ("enableLineBridge".into(), serde_json::json!(true)),
            (
                "lineChannelAccessToken".into(),
                serde_json::json!("line-test-token"),
            ),
            (
                "lineChannelSecret".into(),
                serde_json::json!("line-test-secret"),
            ),
            ("lineWebhookPort".into(), serde_json::json!(address.port())),
            ("lineWebhookHost".into(), serde_json::json!("127.0.0.1")),
            ("enableWhatsappBridge".into(), serde_json::json!(true)),
            (
                "whatsappCloudAccessToken".into(),
                serde_json::json!("wa-test-token"),
            ),
            (
                "whatsappVerifyToken".into(),
                serde_json::json!("verify-challenge"),
            ),
            (
                "whatsappAppSecret".into(),
                serde_json::json!("wa-test-secret"),
            ),
            (
                "whatsappWebhookPort".into(),
                serde_json::json!(address.port()),
            ),
            ("whatsappWebhookHost".into(), serde_json::json!("127.0.0.1")),
        ]);
    })
}

fn signed_header(secret: &str, body: &[u8], line: bool) -> String {
    use base64::Engine as _;
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    if line {
        format!(
            "X-Line-Signature: {}\r\n",
            base64::engine::general_purpose::STANDARD.encode(digest)
        )
    } else {
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        format!("X-Hub-Signature-256: sha256={hex}\r\n")
    }
}

#[test]
fn core_migration_signed_webhooks_share_api_port_without_bypassing_api_auth() {
    let server = webhook_server();
    for (path, secret, body, line) in [
        (
            "/callback",
            "line-test-secret",
            br#"{"events":[]}"#.as_slice(),
            true,
        ),
        ("/", "wa-test-secret", br#"{"entry":[]}"#.as_slice(), false),
    ] {
        let signature = signed_header(secret, body, line);
        let (status, _) = server.request("POST", path, &signature, body);
        assert_eq!(status, "HTTP/1.1 200 OK", "signed {path} webhook");
        let (status, _) = server.request("POST", path, &signature, br#"{"altered":true}"#);
        assert_eq!(
            status, "HTTP/1.1 401 Unauthorized",
            "tampered {path} webhook"
        );
    }
    assert_eq!(
        server.request("GET", "/api/status", "", &[]).0,
        "HTTP/1.1 401 Unauthorized"
    );
}

#[test]
fn core_migration_whatsapp_challenge_requires_subscribe_and_correct_verify_token() {
    let server = webhook_server();
    let path = "/?hub.mode=subscribe&hub.verify_token=verify-challenge&hub.challenge=hello";
    let (status, body) = server.request("GET", path, "", &[]);
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(body, "hello");
    let (status, _) = server.request(
        "GET",
        "/?hub.verify_token=verify-challenge&hub.challenge=hello",
        "",
        &[],
    );
    assert_eq!(status, "HTTP/1.1 403 Forbidden");
}

#[test]
fn core_migration_webhook_signature_is_checked_against_exact_body_bytes() {
    let server = webhook_server();
    let body = [0xff];
    let signature = signed_header("line-test-secret", &body, true);
    let (status, _) = server.request("POST", "/callback", &signature, &body);
    assert_eq!(
        status, "HTTP/1.1 400 Bad Request",
        "valid raw signature must reach JSON validation"
    );
}

#[test]
fn core_webhooks_serve_dedicated_ports_when_the_api_uses_another_port() {
    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let webhook = reservation.local_addr().unwrap();
    drop(reservation);
    let server = Server::configured(|config, _, _| {
        config.extra.extend([
            ("enableLineBridge".into(), serde_json::json!(true)),
            (
                "lineChannelAccessToken".into(),
                serde_json::json!("test-token"),
            ),
            ("lineChannelSecret".into(), serde_json::json!("test-secret")),
            ("lineWebhookPort".into(), serde_json::json!(webhook.port())),
            ("lineWebhookHost".into(), serde_json::json!("127.0.0.1")),
        ]);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while TcpStream::connect(webhook).is_err() {
        assert!(Instant::now() < deadline, "dedicated webhook did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let body = br#"{"events":[]}"#;
    let signature = signed_header("test-secret", body, true);
    assert_eq!(
        Server::request_at(webhook, "POST", "/callback", &signature, body).0,
        "HTTP/1.1 200 OK"
    );
    assert_eq!(
        Server::request_at(webhook, "POST", "/callback", "", body).0,
        "HTTP/1.1 401 Unauthorized"
    );
    assert_eq!(
        server.request("GET", "/api/status", "", &[]).0,
        "HTTP/1.1 200 OK"
    );
}

#[test]
fn core_webhooks_reject_missing_and_non_ascii_signatures_without_panicking() {
    let server = webhook_server();
    let body = br#"{"entry":[]}"#;
    assert_eq!(
        server.request("POST", "/webhook", "", body).0,
        "HTTP/1.1 401 Unauthorized"
    );
    let signature = format!("X-Hub-Signature-256: sha256={}\r\n", "ก".repeat(21) + "a");
    assert_eq!(
        server.request("POST", "/webhook", &signature, body).0,
        "HTTP/1.1 401 Unauthorized"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn review_shared_webhook_respects_configured_interface_on_wildcard_api() {
    let server = Server::configured_on("0.0.0.0", |config, address, _| {
        config.extra.extend([
            ("apiAuthToken".into(), serde_json::json!("test-api-secret")),
            ("enableLineBridge".into(), serde_json::json!(true)),
            (
                "lineChannelAccessToken".into(),
                serde_json::json!("test-token"),
            ),
            ("lineChannelSecret".into(), serde_json::json!("test-secret")),
            ("lineWebhookPort".into(), serde_json::json!(address.port())),
            ("lineWebhookHost".into(), serde_json::json!("127.0.0.2")),
        ]);
    });
    let body = br#"{"events":[]}"#;
    let signature = signed_header("test-secret", body, true);
    let allowed = SocketAddr::from(([127, 0, 0, 2], server.address.port()));
    assert_eq!(
        Server::request_at(allowed, "POST", "/callback", &signature, body).0,
        "HTTP/1.1 200 OK"
    );
    let other_interface = SocketAddr::from(([127, 0, 0, 1], server.address.port()));
    assert_eq!(
        Server::request_at(other_interface, "POST", "/callback", &signature, body).0,
        "HTTP/1.1 401 Unauthorized",
        "callback escaped its configured interface"
    );
    // Explicit wildcard configuration still permits callbacks on every covered interface.
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(server.root.join("config.json")).unwrap()).unwrap();
    config["lineWebhookHost"] = serde_json::json!("0.0.0.0");
    std::fs::write(
        server.root.join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Server::request_at(other_interface, "POST", "/callback", &signature, body).0,
        "HTTP/1.1 200 OK"
    );
}
