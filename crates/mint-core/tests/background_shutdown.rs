use mint_core::{MintConfig, bg_shell};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[test]
fn backend_shutdown_stops_owned_jobs_and_rejects_new_starts() {
    let config = MintConfig {
        safety_enabled: false,
        sandbox_mode: "off".into(),
        ..Default::default()
    };
    let first = bg_shell::start_background(Path::new("."), &config, "sleep 30").unwrap();
    let second = bg_shell::start_background(Path::new("."), &config, "sleep 30").unwrap();
    let started = Instant::now();
    bg_shell::shutdown();
    assert!(started.elapsed() < Duration::from_secs(4));
    for id in [first.id, second.id] {
        assert_eq!(bg_shell::job_json(&id, false).unwrap()["status"], "stopped");
    }
    assert_eq!(bg_shell::running_count(), 0);
    assert!(bg_shell::start_background(Path::new("."), &config, "echo forbidden").is_err());
}
