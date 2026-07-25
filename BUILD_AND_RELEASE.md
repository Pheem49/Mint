# Build and Release

## ข้อ 1 Validate

```bash
npm ci
npm run build:web
cargo test -p mint-core -p mint-cli -p mint-desktop
cargo check -p mint-desktop
```

## ข้อ 2 Build Desktop Bundles

```bash
npm run tauri:build
```

Tauri writes platform bundles under `target/release/bundle/`.

## ข้อ 3 Automated GitHub Release

Pushing a semver tag starts the release workflow:

```bash
git tag v1.10.0
git push origin v1.10.0
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

## ข้อ 4 Signed Updates

The updater requires a configured release endpoint, a public key in the Tauri config, and signed
release artifacts. Exercise update installation against the published endpoint before promoting a
release.

## ข้อ 5 Publish to npm

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





