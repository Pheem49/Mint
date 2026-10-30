# Installing Mint

[← Back to Mint](../README.md)

## <img src="../assets/setup.svg" width="24" height="24" valign="middle" /> Prerequisites

For prebuilt CLI and Desktop downloads, start with [Installation](#installation)
below. The tools listed here are needed when building Mint from source;
native runtime libraries may also be required by your platform.


| Tool | Description | Required For |
| :--- | :--- | :--- |
| **Node.js & npm** | JavaScript runtime and package manager | Frontend UI (React, Vite, TypeScript) |
| **Rust Toolchain** | Rust compiler (`rustc`) and package manager (`cargo`) | Shared domain logic, CLI, and Tauri backend |
| **System Dependencies** | Native OS libraries (compiler tools, dbus, webkit) | Compiling window GUI, Webview rendering, and OS utilities |

### Linux Dependencies

Install the required C compilers, WebKitGTK, and system libraries for your specific Linux distribution:

**Debian / Ubuntu / Linux Mint:**
```bash
sudo apt-get install -y \
  build-essential curl file pkg-config wget \
  libdbus-1-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev \
  poppler-utils unzip patchelf
```

**Fedora / RHEL / CentOS:**

```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y \
  webkit2gtk4.1-devel openssl-devel curl wget glibc-devel \
  dbus-devel libayatana-appindicator-devel librsvg2-devel \
  alsa-lib-devel \
  poppler-utils unzip patchelf
```

**Arch Linux:**
```bash
sudo pacman -Syu --needed \
  base-devel webkit2gtk-4.1 openssl curl wget \
  dbus libayatana-appindicator librsvg \
  alsa-lib \
  poppler unzip patchelf
```

> [!NOTE]
> ALSA development headers (`libasound2-dev`/`alsa-lib-devel`/`alsa-lib`) are required to build `cpal`, used for native microphone capture in the desktop app's voice input feature.

> [!TIP]
> **Other Platforms:** If you are developing on macOS or Windows, follow the official [Tauri Prerequisites Guide](https://v2.tauri.app/start/prerequisites/) to set up your build environment.

## Installation

### Quick Install (Recommended)
The easiest way to install Mint CLI is using our installation script:

**For macOS & Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | bash
```

**For Windows (PowerShell):**
```powershell
powershell -Command "iwr -useb https://raw.githubusercontent.com/Pheem49/Mint/main/install.ps1 | iex"
```

---
### Quick Start
```bash
mint onboard
mint setup
mint
mint web
mint chat "Hello"
mint imagine "A futuristic mint-colored robot" --aspect 16:9
```

Most integrations can be configured from:
```bash
mint onboard
mint setup
mint
```

### Manual Installation

### 1. Configure API Keys
Copy the template and configure your LLM credentials (Gemini, OpenAI, Anthropic, etc.):
```bash
cp .env.example .env
```
Open the `.env` file and insert your API keys (e.g. `GEMINI_API_KEY=your_key_here`).

### 2. Desktop Application
Install dependencies once with `npm install`, then use these four Desktop commands:

| Command | Behavior |
| --- | --- |
| `npm start` | Open the latest Desktop. Changed Rust/UI/build inputs trigger a fast release build; unchanged successful builds launch immediately. Build failures stop without opening an older binary. |
| `npm run dev` | Open Desktop in development mode with UI hot reload. |
| `npm run build:desktop` | Build the fully optimized Desktop and UI without opening it. |
| `npm run package` | Build the optimized Desktop and native installers; Linux also gets a portable tarball. |

The first `npm start` verifies the build once. Successful full builds and packages are reused by later starts. `npm start -- --rebuild` forces a fresh fast build; `npm start -- --release` requires a fully optimized build. Other arguments reach the Desktop executable, and the directory invoking the command remains its working directory.

Old launch/build names remain compatibility aliases. `dev:desktop:ui` and `build:desktop:ui` are UI-only steps used internally by Tauri. `npm run build:ui` builds both Desktop and Web UI assets (`out/renderer` and `out/web`); `npm run build` remains its compatibility alias. CLI and Web commands are separate.

### 3. Native CLI
Pick one way to get the global `mint` command:

* **Release build (recommended — fastest to run):**
  ```bash
  cargo build --release -p mint-cli
  sudo cp target/release/mint /usr/local/bin/
  ```
* **Cargo install:**
  ```bash
  cargo install --path crates/mint-cli
  ```
  *(make sure `~/.cargo/bin` is on your shell's `$PATH`)*
* **Source checkout with automatic rebuilds:** use the Node launcher at `src/bin/index.js` as your `mint` command (requires Node.js and Cargo). On Linux, for example, link it into a directory on your PATH:
  ```bash
  ln -s "$PWD/src/bin/index.js" ~/.local/bin/mint
  ```
  If another `mint` command is earlier on PATH, replace that entry or adjust PATH. Back up an existing executable before replacing it.

From a source checkout, `npm run cli -- <command>` and the linked launcher compare CLI/Core source contents and build inputs with the last successful build. Changed inputs trigger a release build; unchanged inputs launch immediately. A failed build displays the compiler error and stops without opening an old binary. The first launch verifies the build once. `mint`, `mint web`, and `mint browser run` use the directory where you invoke them as their working directory, even when the source checkout lives elsewhere.

`npm run build:cli` still builds explicitly. Cargo-installed standalone binaries and distributed npm packages do not automatically rebuild source changes. `MINT_BIN` explicitly selects a binary and bypasses automatic rebuilding.

---

### First Run — "Unidentified Developer" / "Unknown Publisher"

Prebuilt downloads from the [Releases page](https://github.com/Pheem49/Mint/releases) — the
desktop `.dmg` / `.exe` and the standalone `mint-cli_*` binaries — are **not yet code-signed**,
so macOS Gatekeeper and Windows SmartScreen flag them on first launch. The warnings are
expected and safe to dismiss. Installing with `install.sh` / `install.ps1` uses a verified
prebuilt CLI when the current platform is available in the latest release. Set
`MINT_SOURCE_INSTALL=1` to force the source build. Installing through `npm` still uses the
source build.

**macOS** — Gatekeeper blocks unsigned, un-notarized builds until you clear the quarantine flag:

```bash
# Desktop app
xattr -dr com.apple.quarantine /Applications/Mint.app

# Standalone CLI binary — use the file you actually downloaded
xattr -d com.apple.quarantine ./mint-cli_macos_arm64 && chmod +x ./mint-cli_macos_arm64
```

For the app you can instead right-click `Mint.app` → **Open** → **Open** in the dialog (once).

**Windows** — SmartScreen shows "Windows protected your PC":

1. Click **More info**.
2. Click **Run anyway**.

This appears once per new version until the builds are signed.
