Write-Host "=== Installing Mint CLI via npm ===" -ForegroundColor Green

# Check for Node.js / npm
if ((Get-Command "npm" -ErrorAction SilentlyContinue) -eq $null) {
    Write-Host "Node.js and npm are not installed. They are required to install Mint CLI." -ForegroundColor Yellow
    $response = Read-Host "Would you like to install Node.js and npm automatically? [Y/n]"
    if ($response -eq "" -or $response -like "y*" -or $response -like "Y*") {
        if ((Get-Command "winget" -ErrorAction SilentlyContinue) -ne $null) {
            Write-Host "Installing Node.js via winget..." -ForegroundColor Cyan
            winget install --id OpenJS.NodeJS --exact --silent --accept-package-agreements --accept-source-agreements
        } else {
            Write-Host "winget is not available. Downloading official Node.js MSI installer..." -ForegroundColor Cyan
            $NodeMsi = Join-Path $env:TEMP "node-install.msi"
            Invoke-WebRequest -Uri "https://nodejs.org/dist/v20.11.0/node-v20.11.0-x64.msi" -OutFile $NodeMsi
            Write-Host "Running installer silently. Please wait..." -ForegroundColor Cyan
            Start-Process msiexec.exe -ArgumentList "/i `"$NodeMsi`" /qn /norestart" -Wait
        }
        # Refresh environment PATH for current session
        $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
    } else {
        Write-Error "Installation aborted. Node.js/npm is required."
        exit 1
    }
}

# Check for Rust/Cargo (since the npm package compiles from source on postinstall)
if ((Get-Command "cargo" -ErrorAction SilentlyContinue) -eq $null) {
    Write-Host "Rust/Cargo is not installed. It is required to compile Mint CLI." -ForegroundColor Yellow
    $response = Read-Host "Would you like to install Rust/Cargo automatically? [Y/n]"
    if ($response -eq "" -or $response -like "y*" -or $response -like "Y*") {
        Write-Host "Downloading rustup-init.exe..." -ForegroundColor Cyan
        $RustupInit = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $RustupInit
        Write-Host "Installing Rust silently..." -ForegroundColor Cyan
        Start-Process $RustupInit -ArgumentList "-y" -Wait
        # Refresh environment PATH for current session (specifically adding ~/.cargo/bin)
        $CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
        if ($env:Path -notlike "*$CargoBin*") {
            $env:Path += ";$CargoBin"
        }
    } else {
        Write-Error "Installation aborted. Rust/Cargo is required."
        exit 1
    }
}

Write-Host "Running npm install -g @pheem49/mint@latest..." -ForegroundColor Cyan
npm install -g @pheem49/mint@latest

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "=== Mint CLI Installed Successfully! ===" -ForegroundColor Green
    Write-Host "Type 'mint' to get started." -ForegroundColor Green
} else {
    Write-Error "npm installation failed."
    exit 1
}
