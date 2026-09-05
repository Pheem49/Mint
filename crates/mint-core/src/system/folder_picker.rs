//! Native "choose a folder" dialog, shelled out to the OS's own picker
//! (`zenity`/`kdialog` on Linux, `osascript` on macOS).
//!
//! Used by the Linked Folders "Browse…" button on the **web** UI: `mint web`
//! serves the browser pointed at it from the same machine it runs on, so the
//! dialog opens on the right host. The API route that calls this
//! (`POST /api/select-folder`) is gated to loopback callers so a `mint web`
//! instance reachable from other machines can't be made to pop a dialog on
//! the server. The desktop app has its own Tauri-native picker and does not
//! use this.

use std::process::{Command, Stdio};

/// Opens a directory picker and returns the chosen absolute path, or `None`
/// if the user cancelled, no picker program is installed, or there's no
/// display to show a dialog on. Blocking — call it from `spawn_blocking` in
/// an async context.
pub fn pick_directory_blocking() -> Option<String> {
    for (program, args) in picker_candidates() {
        let Ok(output) = Command::new(program)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        else {
            // Program isn't installed — fall through to the next candidate.
            continue;
        };
        if !output.status.success() {
            // A non-zero exit from a picker that *did* run almost always
            // means the user hit Cancel (zenity and osascript both exit 1
            // on cancel), not that it's the wrong program — so stop here
            // rather than falling through to another dialog.
            return None;
        }
        let picked = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return if picked.is_empty() {
            None
        } else {
            Some(picked)
        };
    }
    None
}

#[cfg(target_os = "macos")]
fn picker_candidates() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![(
        "osascript",
        vec![
            "-e",
            "POSIX path of (choose folder with prompt \"Select a folder to link\")",
        ],
    )]
}

#[cfg(not(target_os = "macos"))]
fn picker_candidates() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "zenity",
            vec![
                "--file-selection",
                "--directory",
                "--title=Select a folder to link",
            ],
        ),
        ("kdialog", vec!["--getexistingdirectory", "."]),
    ]
}
