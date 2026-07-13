# Build and Release

## Validate

```bash
npm ci
npm run build:web
cargo test -p mint-core -p mint-cli -p mint-desktop
cargo check -p mint-desktop
```

## Build Desktop Bundles

```bash
npm run tauri:build
```

Tauri writes platform bundles under `target/release/bundle/`.

## Automated GitHub Release

Pushing a semver tag starts the release workflow:

```bash
git tag v1.8.3
git push origin v1.8.3
```

The workflow builds Linux release artifacts on GitHub Actions and publishes them
to the tagged GitHub Release:

- Debian package from `target/release/bundle/deb/*.deb`
- Portable desktop tarball from `target/release/bundle/tar/*.tar.gz`
- CLI binary as `mint-cli-linux-x86_64`
- `SHA256SUMS`

The same workflow can be started manually from the Actions tab with
`workflow_dispatch`; manual runs upload workflow artifacts but only tag-triggered
runs publish a GitHub Release.

## Signed Updates

The updater requires a configured release endpoint, a public key in the Tauri config, and signed
release artifacts. Exercise update installation against the published endpoint before promoting a
release.

## Publish to npm

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

## Publish to Flathub (Flatpak)

Flathub packages are managed via the [flatpak/](file:///home/pheem49/vscode/Project/Mint-CLI/flatpak/) files. To build and test your Flatpak locally:

1. **Install Flatpak builder tools**:
   ```bash
   sudo apt install flatpak-builder
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47 org.freedesktop.Sdk.Extension.rust-stable//24.08
   ```

2. **Build the React Frontend UI on host machine**:
   ```bash
   npm install
   npm run build:desktop:ui
   ```

3. **Clone Flathub Shared Modules (needed for System Tray Icon support)**:
   ```bash
   git clone https://github.com/flathub/shared-modules.git
   ```

4. **Build the Flatpak locally**:
   ```bash
   flatpak-builder --force-clean build-dir flatpak/com.pheem49.mint.yaml
   ```

5. **Run the built Flatpak**:
   ```bash
   flatpak-builder --run build-dir flatpak/com.pheem49.mint.yaml mint-desktop
   ```

*Note: For official Flathub submission, you must generate offline dependencies using `flatpak-node-generator` and `flatpak-cargo-generator` and reference them in the manifest.*


