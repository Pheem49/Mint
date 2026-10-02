---
name: cargo-clean-decision
description: Decide whether running `cargo clean` is worth it — measure the target/ breakdown and disk headroom first, check what depends on release artifacts (npm shims, installed CLIs), and choose the cheapest targeted cleanup instead of nuking everything. Trigger when the user asks 'should I run cargo clean?', 'target/ is eating my disk', or how to reclaim disk space from a Rust build.
revisions: 1
---

# Cargo Clean Decision

"Should I run `cargo clean`?" is a decision, not a yes/no reflex. A full `cargo clean` deletes *every* profile's artifacts, including release binaries that installed CLIs and package shims spawn — so the answer is usually **"not a full clean, and not now; do a targeted one after your tests pass."** Measure first, then pick the cheapest option that reclaims the space.

## Core stance

- **`cargo clean` (no args) nukes `target/` entirely** — debug *and* release, all crates. That is the most expensive option and rarely what the user wants.
- **Measure before recommending.** The user *feels* target/ is big; compute how big, what's inside, and how much disk is actually free. A 17G target on a 389G-used disk is not the cause of the problem, and saying so is more useful than cleaning blindly.
- **Find out what depends on the artifacts before deleting them.** A release binary the user's installed CLI spawns, or a package shim, turns a "free" clean into a broken command that needs a slow rebuild.
- **The cheapest cleanup wins.** Prefer targeted deletion over profile-wide over full.

## Step 1 — Measure

```bash
du -h -d1 target/                       # per-profile sizes (debug, release, …)
du -sh target/debug/incremental \
       target/debug/deps \
       target/debug/build \
       target/release 2>/dev/null
df -h .                                 # actual free space + % used
```

Typical shape you'll find:

```
target/                     17G
├─ target/debug/            15G
│  ├─ incremental/         7.2G   ← pure incremental cache, safest to delete
│  ├─ deps/                7.4G   ← compiled dependency artifacts (expensive to rebuild)
│  └─ build/               326M   ← build-script outputs
└─ target/release/         1.1G   ← the binary users actually run
```

Also check:

- **Fallback `target/` dirs.** Workspaces often have one at the root only, but check `crates/*/target` and `src-tauri/target` too — don't assume.
- **`~/.cargo/registry/` and `~/.cargo/git/`** (downloaded crate sources). These are *outside* `target/` — **`cargo clean` does not touch them.** Freeing them is a separate operation.
- **Percent free, not just GB free.** The decision hinges on `df`'s use%:
  - `>10%` free → headroom is fine; cleaning is optional.
  - `~5-10%` free → borderline; watch it, targeted cleanup is reasonable.
  - `<5%` free → act; this is when clean is genuinely warranted.

## Step 2 — Find what depends on the release artifacts

Before recommending anything that removes `target/release/`, check whether a release binary is load-bearing:

- **npm / package shims.** A `package.json` `bin` entry or a `src/bin/index.js` that `spawn`s `target/release/<binary>` means deleting the release artifacts breaks the installed command until the user rebuilds with `--release`. Grep for `target/release` in the repo.
- **Installed CLIs.** If the user `cargo install`ed the crate, the binary lives in `~/.cargo/bin` (a copy), but if they run it straight out of `target/release`, cleaning kills it.
- **Anything a service/daemon currently runs.** A running process holding the binary means you must not delete it mid-run anyway.

If a release binary is load-bearing, **any option that deletes `target/release/` is off the table** until the user accepts the rebuild.

## Step 3 — Pick a tiered option

Order by (space reclaimed) vs (real cost):

| Option | Reclaims | Cost | Notes |
|---|---|---|---|
| `cargo clean` | everything | **rebuild debug + release from scratch** | Worst option. Only when disk is critically full or artifacts are invalid anyway. |
| `cargo clean --profile dev` | debug profile (~15G) | rebuild debug deps next build; **release survives** | Good when release binary matters. |
| Delete `target/debug/incremental/` only | the incremental cache (~7.2G) | **deps/ still usable** — recompiles only workspace crates, non-incrementally | Cheapest real reclaim. `cargo clean` has no subcommand for this — delete the folder manually. |
| `cargo clean -p <pkg> -p <pkg>` | just those crates' artifacts | rebuild those crates | When only one crate's artifacts are stale/huge. |
| `CARGO_INCREMENTAL=0` on future builds | prevents regrowth | slower builds | Use after trimming incremental to keep it from ballooning back. |

Notes that make the recommendation honest:

- `cargo clean --profile dev` and `-p` are the two sub-commands worth knowing; `--profile` removes a whole profile, `-p` scopes to packages.
- There is **no** `cargo clean --incremental`; the incremental cache is a plain directory — delete it by hand or via a file manager.
- `~/.cargo/registry` needs a separate tool (`cargo cache`, or manual prune) — say so, don't imply `cargo clean` covers it.

## Step 4 — Criteria: when clean *is* warranted

**Clean when:**
1. A build already failed with `No space left on device`.
2. Free space is below ~5% and dropping.
3. The user just ran `rustup update` / switched toolchains — the old artifacts are unusable anyway, so a clean is effectively free.
4. Measuring true clean-build time, or prepping the repo before backup/archive.
5. Preparing for a big release build (mostly for peace of mind; modern cargo manages freshness itself).

**Don't clean when:**
- The user is mid-iteration, or has an uncommitted/unpushed build-test-push queue in flight — a rebuild burns time and (on RAM-limited machines) risks OOM.
- The user only *feels* the disk is full. Reclaiming a few GB when the real consumers are elsewhere is theater. Point them at the real culprits (see Gotchas).

## Step 5 — Safety rails

- **Never clean while a build is running.** Deleting artifacts mid-compile corrupts them in hard-to-recover ways. Check first:

  ```bash
  pgrep -a cargo
  pgrep -a rustc
  ```

  If anything is running, stop and tell the user; don't proceed.
- **Don't `rm -rf` on the user's behalf.** Hand over the exact command (or ask for explicit approval) rather than running destructive deletes unasked. Deleting the incremental folder manually is a `rm` the user should own.
- **State the recovery path** for what you delete: e.g. deleting `target/release/` is undone by `cargo build --release`.

## Output shape

1. **Short verdict up front** — usually "you don't need a full `cargo clean`; here's the targeted option."
2. **Measured breakdown** — the `target/` tree with sizes + `df` free/% used, labeled as read from the machine.
3. **Why not a full clean** — call out any load-bearing release binary (npm shim / installed CLI) and the rebuild cost (dependency count, RAM limits).
4. **Options table** — space reclaimed vs real cost, cheapest-first.
5. **Criteria** — the conditions under which they *should* clean.
6. **Point at the real disk hogs** — if target/ is a small fraction of used space, list where else to look (next section).
7. **Offer A/B/C next steps** and ask which — clean, run the pending tests first, or hunt the real space consumers.

## Gotchas

- **A full `cargo clean` deletes release binaries that npm shims and installed CLIs spawn.** Grep for `target/release` in the repo before recommending it — this is the top way a "free cleanup" breaks the user's setup.
- **`~/.cargo/registry` survives `cargo clean`.** If the user expects the cache gone, it isn't — that's a separate prune.
- **`target/` is rarely the cause of a full disk.** It's usually a few percent of used space; the real hogs are `node_modules`, Docker (`docker system df` / overlay2), Trash, snap revisions, and *other* Rust projects' `target/` dirs. Name them rather than letting a 17G clean feel like the fix.
- **There is no `cargo clean --incremental`.** The incremental cache is deleted by hand; saying otherwise sends the user looking for a flag that doesn't exist.
- **On RAM-limited machines** (see workspace build notes), a forced rebuild of a large dependency tree can OOM — factor that into the cost, and build one command at a time.
- **Cleaning right before a push/test queue** is the worst timing: you pay the rebuild to run a gate you could have run on the warm cache first.
- **Recommend the sequence, not just the action:** finish the pending test/build gate *first*, *then* clean if space still matters.
- **Answer in the user's language.** Thai trigger ("ฉันควรรัน cargo clean มั้ย") → reply in Thai.