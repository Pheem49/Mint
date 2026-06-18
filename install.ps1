Write-Host "=== Installing Mint CLI ===" -ForegroundColor Green

# Check for Cargo
if ((Get-Command "cargo" -ErrorAction SilentlyContinue) -eq $null) {
    Write-Error "Rust/Cargo is not installed. Please install Rust first: https://www.rust-lang.org/tools/install"
    exit 1
}

# Create temp dir
$TempDir = Join-Path $env:TEMP "Mint-Build-$(Get-Random)"
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

Write-Host "Cloning Mint repository..." -ForegroundColor Cyan
git clone https://github.com/Pheem49/Mint.git $TempDir

Push-Location $TempDir

Write-Host "Building Mint CLI in release mode..." -ForegroundColor Cyan
cargo build --release -p mint-cli

# Target binary path
$InstallDir = "$env:USERPROFILE\.mint\bin"
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

Write-Host "Installing binary to $InstallDir..." -ForegroundColor Cyan
Copy-Item "target\release\mint.exe" "$InstallDir\mint.exe" -Force

# Add to user Path if not present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to user Path..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path += ";$InstallDir"
}

Pop-Location
Remove-Item -Recurse -Force $TempDir

Write-Host "=== Mint CLI Installed Successfully! ===" -ForegroundColor Green
Write-Host "Please restart your terminal/PowerShell session, then type 'mint' to get started." -ForegroundColor Yellow
