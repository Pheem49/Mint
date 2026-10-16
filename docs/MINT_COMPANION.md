# Mint Companion

Mint Companion is an optional Tauri application in the same repository. Mint Desktop and Web no longer bundle the Cubism engine or Shiroko assets. Project Avatar (`/avatar`) remains a separate integration; the existing desktop widget and proactive suggestions stay in Mint.

## Run and package

From the repository root, install with `npm ci`, open Mint Desktop, CLI, or the Web backend, then run `npm run dev:companion`. Companion uses Vite port 9001; Mint uses 9000. For an optimized binary use `npm run build:companion` and run `target/release/mint-companion` (`.exe` on Windows). `npm run package:companion -- --bundles deb`, `nsis`, or `dmg` creates the platform installer. Release workflows collect Mint bundles before building Companion so filenames cannot collide; Linux also includes a portable binary tarball. Companion embeds its frontend and character assets. The npm CLI package retains the Rust workspace manifests and source needed by its postinstall build, but omits the Companion character assets. A packaging regression test resolves Cargo metadata from the actual npm file list.

Choose a Mint instance and conversation in the Chat panel. A single available instance is selected automatically on first use; multiple instances require a selection. Selection does not follow unrelated new tasks. After a Mint process exits, select its new instance when it restarts. Companion cannot connect to a remote Web server in v1.

The transparent floating window can be dragged using its title. Settings provide a normal opaque window, always-on-top toggle, character size, manual/automatic expression, accessories, interaction lock, and clickable areas. Hide restores from the tray's Show Companion menu. Quit exits Companion only. Preferences and window geometry use the user config directory under `mint-companion/settings.json`, independently of Mint's settings. Hidden/minimized windows stop rendering; reduced-motion preferences disable status gestures. Models that fail to load leave Chat and Settings usable, with a retry action.

## Transport and lifecycle

Each running Mint process starts one optional WebSocket listener on `127.0.0.1` with an ephemeral port. Its user-private descriptor in the config directory under `mint/companion/hosts/<host-id>.json` carries a random token and protocol version. Unix directories/files use 0700/0600; Windows uses the inherited user-profile permissions. Descriptor cleanup runs on orderly shutdown; stale descriptors after crashes are ignored by authenticated discovery. No integrations or scheduler are started by this listener.

The native Companion client authenticates using a Bearer header. Browser Origin requests, wrong tokens, wrong paths, and non-loopback descriptors are rejected. Tokens remain in Rust; the frontend receives only host names/IDs and session data. Protocol types live in `mint-companion-protocol`; the Companion executable does not depend on Mint Core, provider configuration, or SQLite.

Protocol v1 commands are `list_sessions`, `subscribe {chatId}`, `send_chat {chatId, requestId, message}`, and `interact {chatId, requestId, area}`. Events are `snapshot`, `update`, `accepted`, and `error`. Snapshots carry protocol version, host ID, session metadata, and turns for the selected session. Updates identify host/chat/turn and a monotonic sequence. Turns expose queued, thinking, working, responding, waiting, completed, failed, or interrupted state and the user-facing reply; reasoning and raw tool inputs/results are omitted.

Host orchestration publishes the lifecycle centrally for all interfaces. Nested subagent sessions do not take over the character. Queued turns do not replace the current active turn. Plain chat and interaction prompts use the existing orchestrator, fallback providers, SQLite memory, and cross-process TurnLease queue. The host supplies interaction instructions and never accepts client tools or system instructions. Tools and approvals remain in the main Mint interface.

Requests use UUIDs and are deduplicated during the host process lifetime, with a bounded 4096-request registry. Messages are limited to 16 KiB. Accepted work continues when Companion disconnects. Reconnection backs off from 1 to 30 seconds and re-subscribes for a fresh snapshot; submitted messages are never automatically resent. If connection drops before confirmation, completion is uncertain: inspect the shared conversation before manually sending again. Host disappearance or missing sessions disables sending rather than routing to another conversation. Protocol mismatches require matching app versions.

## Validation

Run `npm run typecheck`, `npm run typecheck:companion`, `npm run test:companion`, and `npm test`; build each frontend and native app separately. Transport tests use real authenticated loopback sockets and require network-capable test execution. Full native builds are sequential on low-memory machines.

CI compiles/tests on Linux and release jobs package Linux x86_64, Windows x64, and macOS arm64. Build success is separate from GUI validation: transparent windows, topmost behavior, tray restoration, dragging, minimize suspension, and scale/position restoration require a smoke test on each actual desktop environment. v1 uses the existing Shiroko expressions and procedural gestures, with no imported models, speech, lip sync, automatic backend startup, or automatic OS login startup.
