<#
.SYNOPSIS
Mint CLI Installation Script for Windows
.DESCRIPTION
Installs the Mint CLI package globally via NPM.
#>

$ErrorActionPreference = "Stop"

Write-Host "Starting Mint CLI installation..." -ForegroundColor Cyan

# 1. Check for Node.js
try {
    $nodeVersion = (node -v) 2>$null
    if ([string]::IsNullOrWhiteSpace($nodeVersion)) {
        Write-Host "[Error] Node.js is not found on your system!" -ForegroundColor Red
        Write-Host "Please install Node.js (version 18 or higher) before using Mint CLI." -ForegroundColor Yellow
        Write-Host "Download at: https://nodejs.org/"
        exit 1
    }
} catch {
    Write-Host "[Error] Node.js is not found on your system!" -ForegroundColor Red
    Write-Host "Please install Node.js (version 18 or higher) before using Mint CLI." -ForegroundColor Yellow
    Write-Host "Download at: https://nodejs.org/"
    exit 1
}

# 2. Check for NPM
try {
    $npmVersion = (npm -v) 2>$null
    if ([string]::IsNullOrWhiteSpace($npmVersion)) {
        Write-Host "[Error] NPM is not found on your system!" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "[Error] NPM is not found on your system!" -ForegroundColor Red
    exit 1
}

# Check Node.js version (recommend 18+)
$majorVersion = [int]($nodeVersion.Substring(1).Split('.')[0])
if ($majorVersion -lt 18) {
    Write-Host "[Warning] Your Node.js version is too old (current: $nodeVersion)" -ForegroundColor Yellow
    Write-Host "It is recommended to update to version 18 or higher for full functionality." -ForegroundColor Yellow
}

# 3. Install Mint CLI via NPM
Write-Host "Downloading and installing @pheem49/mint..." -ForegroundColor Cyan

try {
    # In Windows, we usually don't need sudo for global npm install if Node was installed for the user.
    # We will just run the command normally.
    npm install -g @pheem49/mint@latest
} catch {
    Write-Host "[Error] Installation failed. Try running PowerShell as Administrator." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Mint CLI installed successfully!" -ForegroundColor Green
Write-Host "- Run 'mint' to get started."
Write-Host "- Run 'mint onboard' to set up your API Keys."
Write-Host "----------------------------------------"
Write-Host "Happy coding!" -ForegroundColor Cyan
