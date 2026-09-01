<!--
Maintainer template for the GitHub Release body.

`release.yml` publishes `Release_Note.md` verbatim as the release body
(softprops/action-gh-release `body_path: Release_Note.md`). There is no
separate template file the workflow reads — this file is a reference for
writing `Release_Note.md` when cutting a release.

When releasing:
1. Replace the title + summary + change sections at the top of
   `Release_Note.md` with this version's notes.
2. KEEP the "First Run — Unsigned Build Warnings" footer below verbatim. It
   applies to every release until the Windows/macOS builds are code-signed
   (see BUILD_AND_RELEASE.md). Delete it only once signing + notarization
   land in `release.yml` and `tauri.conf.json`.
-->

# Release Notes - Mint Agent vX.Y.Z

<One short paragraph: the theme of the release and the headline changes.>

---

## <emoji> <Section title per notable change>

<What changed and why it matters. One section per notable item.>

---

<!-- ===== Boilerplate footer — carry forward every release until builds are signed ===== -->

## 🔓 First Run — Unsigned Build Warnings

The desktop installers (`mint-agent_windows_x64.exe`, `mint-agent_macos_arm64.dmg`)
and the standalone `mint-cli_*` binaries are **not yet code-signed**, so the OS
warns you the first time you open them. This is expected. Installing via
`install.sh` / `install.ps1` / `npm` builds from source and is unaffected.

**macOS** — clear the quarantine flag:

```bash
xattr -dr com.apple.quarantine /Applications/Mint.app     # desktop app
xattr -d com.apple.quarantine ./mint-cli_macos_arm64      # CLI binary
```

Or right-click the app → **Open** → **Open**.

**Windows** — on the SmartScreen prompt: **More info** → **Run anyway**.
