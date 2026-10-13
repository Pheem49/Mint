use std::process::Command;

#[test]
fn mod_export_requires_replace_and_preserves_quoted_paths() {
    let root = std::env::temp_dir().join(format!(
        "mint-mod-export-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dir = root.join("mint-mods/sample");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"id":"sample","name":"Sample","version":"1","sdkVersion":1,"entry":"main.ts"}"#,
    )
    .unwrap();
    std::fs::write(dir.join("main.ts"), "export function register(mint) {}").unwrap();
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
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mint"))
            .args(args)
            .current_dir(&root)
            .env("MINT_CONFIG_PATH", root.join("config.json"))
            .env("XDG_DATA_HOME", root.join("data"))
            .output()
            .unwrap()
    };
    let filename = "sample's exported file.mintmod";
    std::fs::write(root.join(filename), "keep this").unwrap();
    assert!(
        !run(&["mods", "export", "sample", filename])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(root.join(filename)).unwrap(),
        "keep this"
    );
    let replaced = run(&["mods", "export", "sample", filename, "--replace"]);
    assert!(
        replaced.status.success(),
        "{}",
        String::from_utf8_lossy(&replaced.stderr)
    );
    assert!(mint_mod_runtime::unpack(&std::fs::read(root.join(filename)).unwrap()).is_ok());
    assert!(
        run(&["mods", "export", "sample", "--replace"])
            .status
            .success()
    );
    assert!(root.join("sample.mintmod").exists());
    assert!(!root.join("--replace").exists());
    let outside = root.with_extension("outside.mintmod");
    assert!(
        !run(&["mods", "export", "sample", outside.to_str().unwrap()])
            .status
            .success()
    );
    assert!(!outside.exists());
    // A package exported from a legacy mod installs into the new directory.
    std::fs::rename(root.join("mint-mods"), root.join("legacy-backup")).unwrap();
    let imported = run(&["mods", "import", filename]);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert!(root.join(".mint-mods/sample/main.ts").is_file());
    assert!(!root.join("mint-mods").exists());
    assert!(
        run(&["mods", "export", "sample", "hidden.mintmod"])
            .status
            .success()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mod_test_command_reports_failures_with_a_nonzero_exit_code() {
    let root = std::env::temp_dir().join(format!(
        "mint-mod-cli-tests-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dir = root.join("mint-mods/sample/tests");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(root.join("config.json"), "{}").unwrap();
    std::fs::write(
        dir.parent().unwrap().join("manifest.json"),
        r#"{"id":"sample","name":"Sample","version":"1","sdkVersion":1,"entry":"main.ts"}"#,
    )
    .unwrap();
    std::fs::write(dir.parent().unwrap().join("main.ts"),"export function register(mint){mint.commands.register('add',()=>mint.state.set('added',true));}").unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_mint"))
            .args(["mods", "test", "sample"])
            .current_dir(&root)
            .env("MINT_CONFIG_PATH", root.join("config.json"))
            .output()
            .unwrap()
    };
    std::fs::write(dir.join("sample.test.ts"),"export function registerTests(test){test('fails',({equal})=>equal(1,2,'Expected update'));}").unwrap();
    let failed = run();
    assert!(
        !failed.status.success(),
        "{}",
        String::from_utf8_lossy(&failed.stdout)
    );
    assert!(String::from_utf8_lossy(&failed.stdout).contains("Expected update"));
    std::fs::write(dir.join("sample.test.ts"),"export function registerTests(test){test('passes',async({command,state,equal})=>{await command('add');equal(state('added'),true);});}").unwrap();
    let passed = run();
    assert!(
        passed.status.success(),
        "{}",
        String::from_utf8_lossy(&passed.stderr)
    );
    assert!(String::from_utf8_lossy(&passed.stdout).contains("passes"));
    std::fs::remove_dir_all(root).unwrap();
}
