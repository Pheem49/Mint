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

## Signed Updates

The updater requires a configured release endpoint, a public key in the Tauri config, and signed
release artifacts. Exercise update installation against the published endpoint before promoting a
release.
