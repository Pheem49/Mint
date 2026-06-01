import os
import sys
import subprocess
import glob

def find_rlib(deps_dir, crate_name):
    normalized = crate_name.replace('-', '_')
    pattern = os.path.join(deps_dir, f"lib{normalized}-*.rlib")
    files = glob.glob(pattern)
    if not files:
        pattern_simple = os.path.join(deps_dir, f"lib{normalized}.rlib")
        files = glob.glob(pattern_simple)
    if not files:
        return None
    files.sort(key=os.path.getmtime, reverse=True)
    return files[0]

def run_rustc(source_file, crate_type, out_dir, deps_dir, externs, extra_flags=[]):
    args = [
        "rustc",
        "--edition", "2024",
        "--crate-type", crate_type,
        source_file,
        "-L", deps_dir,
        "-L", f"dependency={deps_dir}",
    ]
    if out_dir:
        args += ["--out-dir", out_dir]
    
    for crate_name in externs:
        rlib = find_rlib(deps_dir, crate_name)
        if not rlib:
            print(f"Warning: Could not find rlib for {crate_name} in {deps_dir}", file=sys.stderr)
            continue
        args += ["--extern", f"{crate_name.replace('-', '_')}={rlib}"]
    
    args += extra_flags
    
    print("Running command:")
    print(" ".join(args))
    print("-" * 40)
    
    env = os.environ.copy()
    env["CARGO_PKG_VERSION"] = "2.0.0-alpha.1"
    res = subprocess.run(args, capture_output=True, text=True, env=env)
    if res.returncode != 0:
        print("STDOUT:")
        print(res.stdout)
        print("STDERR:")
        print(res.stderr)
        return False, res.stderr
    else:
        print("Compilation successful!")
        return True, ""

def main():
    workspace_dir = "/home/pheem49/vscode/Project/Mint-CLI"
    deps_dir = os.path.join(workspace_dir, "target/debug/deps")
    scratch_out = "/home/pheem49/vscode/Project/Mint-CLI/target/debug/onboard_scratch"
    os.makedirs(scratch_out, exist_ok=True)
    
    core_externs = [
        "base64",
        "dirs",
        "futures-util",
        "quick-xml",
        "regex",
        "reqwest",
        "rusqlite",
        "serde",
        "serde_json",
        "sha2",
        "thiserror",
        "tokio",
    ]
    core_source = os.path.join(workspace_dir, "crates/mint-core/src/lib.rs")
    
    print("=== Compiling mint-core ===")
    ok, core_err = run_rustc(core_source, "lib", scratch_out, deps_dir, core_externs, ["--crate-name", "mint_core"])
    if not ok:
        sys.exit(1)
        
    core_rlib = glob.glob(os.path.join(scratch_out, "libmint_core*.rlib"))
    if not core_rlib:
        core_rlib = glob.glob(os.path.join(scratch_out, "libmint_core.rlib"))
    if not core_rlib:
        print("Error: Could not find compiled libmint_core.rlib in scratch output.")
        sys.exit(1)
    core_rlib_path = core_rlib[0]
    
    cli_externs = [
        "anyhow",
        "base64",
        "clap",
        "dirs",
        "serde",
        "serde_json",
        "tokio",
        "crossterm",
        "reqwest",
        "sha2",
        "chrono",
    ]
    cli_source = os.path.join(workspace_dir, "crates/mint-cli/src/main.rs")
    
    extra_flags = [
        "--extern", f"mint_core={core_rlib_path}",
        "--crate-name", "mint"
    ]
    
    print("\n=== Compiling mint-cli ===")
    ok, cli_err = run_rustc(cli_source, "bin", scratch_out, deps_dir, cli_externs, extra_flags)
    if not ok:
        sys.exit(1)
        
    print("\nAll compiles completed successfully!")

if __name__ == "__main__":
    main()
