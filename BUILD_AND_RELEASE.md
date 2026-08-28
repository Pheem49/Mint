# 🚀 Build and Release

## ✅ ข้อ 1 Validate

```bash
npm ci
npm run build:web
cargo test -p mint-core -p mint-cli -p mint-desktop
cargo check -p mint-desktop
```

## 📦 ข้อ 2 Build Desktop Bundles

```bash
npm run tauri:build
```

Tauri writes platform bundles under `target/release/bundle/`.

## 🚀 ข้อ 3 Automated GitHub Release

Pushing a semver tag starts the release workflow:

```bash
git tag v1.13.0
git push origin v1.13.0
```

The workflow runs one job per platform on GitHub Actions and publishes their
artifacts to the same tagged GitHub Release. Every asset is renamed to
`mint-agent_<platform>_<arch>.<ext>` before upload:

| Platform | Job | Build command | Published asset |
| --- | --- | --- | --- |
| Linux (`ubuntu-latest`) | `linux` | `npm run tauri:build` | `mint-agent_linux_x86_64.deb`, `mint-agent_linux_x86_64.tar.gz` |
| Windows (`windows-latest`) | `windows` | `npx tauri build --bundles nsis` | `mint-agent_windows_x64.exe` |
| macOS (`macos-latest`, arm64) | `macos` | `npx tauri build --bundles dmg` | `mint-agent_macos_arm64.dmg` |

- `mint-agent_linux_x86_64.deb` — Debian package (`target/release/bundle/deb/`).
- `mint-agent_linux_x86_64.tar.gz` — portable bundle of the `mint-desktop`
  binary (`target/release/bundle/tar/`).
- `mint-agent_windows_x64.exe` — NSIS installer (`target/release/bundle/nsis/`).
  Currently **unsigned**, so SmartScreen shows an "unknown publisher" warning
  on first run.
- `mint-agent_macos_arm64.dmg` — disk image containing `Mint.app`
  (`target/release/bundle/dmg/`). **Ad-hoc signed, not notarized** — Gatekeeper
  blocks it until the user right-clicks the app and chooses Open, or runs
  `xattr -cr /Applications/Mint.app`. arm64 only; no Intel build.

The same workflow can be started manually from the Actions tab with
`workflow_dispatch`; manual runs upload workflow artifacts but only tag-triggered
runs publish a GitHub Release.

## 🔐 ข้อ 4 Signed Updates

The updater requires a configured release endpoint, a public key in the Tauri config, and signed
release artifacts. Exercise update installation against the published endpoint before promoting a
release.

## 📤 ข้อ 5 Publish to npm

To publish the repository package to the npm registry as a public scoped package:

1. **Log in to npm** (if not already logged in):
   ```bash
   npm login
   ```

2. **Publish the package** (since `@pheem49/mint` is a scoped package, you must specify public access):
   ```bash

   npm publish --access public
   ```

*Note: You must bump the version number in `package.json` before publishing a new version.*





