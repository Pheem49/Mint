# Mint Release Notes

## v1.5.4

This release introduces automated one-liner installation scripts for macOS, Linux, and Windows, replaces AppImage with `.tar.gz` for Linux packaging, fixes missing dependencies, and improves continuous integration workflows.

### One-Liner Automated Installers

We have introduced automated installation scripts to make setting up and updating Mint CLI easier than ever.

- **Linux & macOS (One-liner):**
  ```bash
  curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | bash
  ```
- **Windows (One-liner via PowerShell):**
  ```powershell
  powershell -Command "iwr -useb https://raw.githubusercontent.com/Pheem49/Mint/main/install.ps1 | iex"
  ```
- **Smart Permission Handling:** The Linux/macOS script automatically detects NVM environments and user write permissions, avoiding unnecessary `sudo` requests when installing the NPM package globally.

### Packaging & CI Improvements

- **Linux Packaging:** Replaced `.AppImage` with `.tar.gz` packaging for better compatibility across different Linux distributions.
- **CI Workflow Update:** Updated the GitHub Actions workflow to run `npm install` instead of `npm ci`, resolving dependency synchronization issues and ensuring smooth builds.

### Dependency & Bug Fixes

- **Missing Dependency:** Added `read-excel-file` to the package dependencies to fix a runtime module error when launching the `mint` command.
