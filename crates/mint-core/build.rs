use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../..");
    let notes = root.join("Release_Note.md");
    let bundle =
        env::var("MINT_BUNDLE_RELEASE_NOTES").as_deref() == Ok("1") || !root.join(".git").exists();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=MINT_BUNDLE_RELEASE_NOTES");
    if bundle {
        println!("cargo:rerun-if-changed={}", notes.display());
    }
    println!(
        "cargo:rustc-env=MINT_LIVE_RELEASE_NOTES={}",
        if bundle { "0" } else { "1" }
    );

    // Keep the snapshot in OUT_DIR: a dev notes edit must not invalidate Rust's
    // include_str dependency. Shipping builds explicitly track the source file.
    let snapshot =
        fs::read(&notes).expect("Release_Note.md is required when creating the snapshot");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("release-notes.md");
    fs::write(output, snapshot).expect("write the release notes snapshot");
}
