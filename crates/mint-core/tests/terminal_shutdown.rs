#![cfg(unix)]

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn shutdown_worker() {
    let Ok(port) = std::env::var("MINT_SHUTDOWN_TEST_PORT") else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (exit, exited) = tokio::sync::oneshot::channel();
        tokio::spawn(mint_core::api_server::shutdown_on_signal(move || {
            let _ = exit.send(());
        }));
        tokio::spawn(async move {
            let result = mint_core::start_api_server(port.parse().unwrap()).await;
            println!("API_STOPPED: {}", result.is_ok());
            std::io::stdout().flush().unwrap();
        });
        // Let the watcher install its signal subscriptions even when the API cannot bind.
        tokio::time::sleep(Duration::from_millis(100)).await;
        println!("READY");
        std::io::stdout().flush().unwrap();
        exited.await.unwrap();
    });
}

fn expect_shutdown(signal: i32, occupied: bool) {
    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    let reservation = occupied.then_some(reservation);
    let config = std::env::temp_dir().join(format!(
        "mint-shutdown-missing-{}.json",
        uuid::Uuid::new_v4()
    ));
    let mut worker = Worker(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shutdown_worker", "--nocapture"])
            .env("MINT_SHUTDOWN_TEST_PORT", port.to_string())
            .env("MINT_CONFIG_PATH", config)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut output = BufReader::new(worker.0.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(
            output.read_line(&mut line).unwrap(),
            0,
            "worker exited before readiness"
        );
        if line.trim() == "READY" {
            break;
        }
    }
    if !occupied {
        std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    }
    assert_eq!(unsafe { libc::kill(worker.0.id() as i32, signal) }, 0);
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = worker.0.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    drop(reservation);
    assert!(
        status.is_some(),
        "terminal signal stopped the embedded API but left the Desktop lifetime running"
    );
    assert!(
        status.unwrap().success(),
        "shutdown callback was bypassed by the OS default signal action"
    );
}

#[test]
fn ctrl_c_exits_the_host_of_an_embedded_api() {
    expect_shutdown(libc::SIGINT, false);
}
#[test]
fn sigterm_exits_the_host_of_an_embedded_api() {
    expect_shutdown(libc::SIGTERM, false);
}
#[test]
fn ctrl_c_exits_the_host_even_when_the_api_port_is_in_use() {
    expect_shutdown(libc::SIGINT, true);
}
