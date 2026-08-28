# Installs the Mint CLI via npm (the npm package compiles `mint-cli` from source
# on postinstall). Ensures every build dependency compile needs is present:
#
#   Required (build fails without them):
#     - Node.js & npm                       install vehicle + frontend build
#     - Rust toolchain (cargo)              compiles mint-core / mint-cli
#     - Visual Studio C++ Build Tools       rusqlite (bundled SQLite) + tree-sitter
#                                           grammars link against the MSVC toolchain
#     (cpal uses WASAPI on Windows - no extra library needed)
#
#   Optional feature tools (installed too, unless $env:MINT_SKIP_OPTIONAL = "1"):
#     - git                                 repo-aware tools
#     - ffmpeg / ffprobe                    video, subtitle and speech tooling
#
#   Not installed here (opt in when you need the feature):
#     - ollama    local LLM provider          https://ollama.com/download
#     - docker    sandboxed shell execution   https://docs.docker.com/desktop/install/windows-install/
#     - a browser (Chrome / Edge)             browser-automation tools

$ErrorActionPreference = "Stop"
$NpmPkg = "@pheem49/mint@latest"
$SkipOptional = $env:MINT_SKIP_OPTIONAL -eq "1"

function Have($name) { $null -ne (Get-Command $name -ErrorAction SilentlyContinue) }

function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                [Environment]::GetEnvironmentVariable("Path", "User")
}

Write-Host "=== Installing Mint CLI ===" -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# 1. Node.js / npm
# ---------------------------------------------------------------------------
Write-Host "--- Checking Node.js / npm ---" -ForegroundColor Cyan
if (-not (Have "npm")) {
    Write-Host "Node.js / npm not found (required to install and manage Mint CLI)." -ForegroundColor Yellow
    $r = Read-Host "Install Node.js automatically? [Y/n]"
    if ($r -eq "" -or $r -like "y*") {
        if (Have "winget") {
            winget install --id OpenJS.NodeJS.LTS --exact --silent `
                --accept-package-agreements --accept-source-agreements
        } else {
            $msi = Join-Path $env:TEMP "node-install.msi"
            Invoke-WebRequest -Uri "https://nodejs.org/dist/v22.11.0/node-v22.11.0-x64.msi" -OutFile $msi -UseBasicParsing
            Start-Process msiexec.exe -ArgumentList "/i `"$msi`" /qn /norestart" -Wait
        }
        Refresh-Path
    } else {
        Write-Error "Node.js/npm is required."; exit 1
    }
}
Write-Host ("OK: node {0} / npm {1}" -f (node --version), (npm --version)) -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# 2. Visual Studio C++ Build Tools (MSVC linker)
# ---------------------------------------------------------------------------
Write-Host "--- Checking Visual Studio C++ Build Tools ---" -ForegroundColor Cyan
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$hasVC = $false
if (Test-Path $vswhere) {
    $vcInstall = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath 2>$null
    if ($vcInstall) { $hasVC = $true }
}
if (-not $hasVC -and (Have "cl")) { $hasVC = $true }

if (-not $hasVC) {
    Write-Host "MSVC C++ Build Tools not found (cargo cannot link rusqlite / tree-sitter without them)." -ForegroundColor Yellow
    $r = Read-Host "Install 'Visual Studio 2022 Build Tools' with the C++ workload now? (~2-3 GB) [Y/n]"
    if ($r -eq "" -or $r -like "y*") {
        if (Have "winget") {
            winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --silent `
                --accept-package-agreements --accept-source-agreements `
                --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
            Refresh-Path
        } else {
            Write-Error ("winget not available. Install the C++ Build Tools manually from " +
                "https://visualstudio.microsoft.com/visual-cpp-build-tools/ and re-run.")
            exit 1
        }
    } else {
        Write-Error "The C++ Build Tools are required to compile Mint CLI."
        exit 1
    }
}
Write-Host "OK: MSVC C++ Build Tools present." -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# 3. Rust toolchain
# ---------------------------------------------------------------------------
Write-Host "--- Checking Rust / Cargo ---" -ForegroundColor Cyan
if (-not (Have "cargo")) {
    Write-Host "Rust / Cargo not found (the npm package compiles Mint CLI from source)." -ForegroundColor Yellow
    $r = Read-Host "Install Rust via rustup? [Y/n]"
    if ($r -eq "" -or $r -like "y*") {
        $rustup = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup -UseBasicParsing
        Start-Process $rustup -ArgumentList "-y --default-host x86_64-pc-windows-msvc" -Wait
        $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
        if ($env:Path -notlike "*$cargoBin*") { $env:Path += ";$cargoBin" }
    } else {
        Write-Error "Rust/Cargo is required."; exit 1
    }
}
Write-Host ("OK: {0}" -f (cargo --version)) -ForegroundColor Green
Write-Host ""

# ---------------------------------------------------------------------------
# 4. Optional feature tools
# ---------------------------------------------------------------------------
if (-not $SkipOptional) {
    Write-Host "--- Installing optional feature tools (set MINT_SKIP_OPTIONAL=1 to skip) ---" -ForegroundColor Cyan
    if (Have "winget") {
        foreach ($id in @("Git.Git", "Gyan.FFmpeg")) {
            winget install --id $id --exact --silent `
                --accept-package-agreements --accept-source-agreements 2>$null
        }
        Refresh-Path
        Write-Host "OK: optional tools processed." -ForegroundColor Green
    } else {
        Write-Host "winget not available - skipping optional tools (git, ffmpeg)." -ForegroundColor Yellow
    }
    Write-Host ""
}

# ---------------------------------------------------------------------------
# 5. Install the CLI
# ---------------------------------------------------------------------------
Write-Host "--- Installing $NpmPkg ---" -ForegroundColor Cyan
npm install -g $NpmPkg
if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "=== Mint CLI installed successfully! ===" -ForegroundColor Green
    if (Have "mint") { mint --version }
    Write-Host "Run 'mint' to get started. (restart your shell if 'mint' isn't found)" -ForegroundColor Green
} else {
    Write-Error "npm installation failed."
    exit 1
}

Write-Host ""
Write-Host "Optional, install only if you use the feature:" -ForegroundColor Cyan
Write-Host "  - ollama   local LLM provider ......... https://ollama.com/download"
Write-Host "  - docker   sandboxed shell execution .. https://docs.docker.com/desktop/install/windows-install/"
Write-Host "  - Chrome / Edge ...................... browser-automation tools"
