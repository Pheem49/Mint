use mint_core::{MintConfig, bg_shell};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[test]
fn retains_latest_hundred_finished_jobs_without_evicting_active_jobs() {
    let config = MintConfig {
        safety_enabled: false,
        sandbox_mode: "off".into(),
        ..Default::default()
    };
    let running = bg_shell::start_background(Path::new("."), &config, "sleep 30").unwrap();
    let mut finished = Vec::new();
    for _ in 0..105 {
        finished.push(
            bg_shell::start_background(Path::new("."), &config, "true")
                .unwrap()
                .id,
        );
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while bg_shell::running_count() > 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(bg_shell::running_count(), 1);
    let list = bg_shell::list_jobs();
    assert_eq!(list.len(), 101);
    assert!(list.iter().any(|job| job.id == running.id));
    assert!(list.iter().any(|job| Some(&job.id) == finished.last()));
    assert!(!list.iter().any(|job| job.id == finished[0]));
    bg_shell::shutdown();
}
