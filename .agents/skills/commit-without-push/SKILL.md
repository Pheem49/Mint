---
name: commit-without-push
description: Create local git commits on request while honoring a 'don't push yet' instruction — group changes into logical commits, follow repo conventions, and never push without explicit instruction; also verify that an already-made commit captured everything (and didn't sweep in unexpected files) when the user asks 'I committed already — nothing left to fix, right?'. Use when the user says 'commit this' / 'commit ให้หน่อย' / 'commit but don't push' / 'commit แยกเป็นคอมมิต', makes any bare request to commit working/staged changes, or asks whether their last commit is complete.
revisions: 5
---

# Commit Without Push

A request to *commit* is never a request to *push*. Treat "don't push yet" (or its absence of any push instruction) as a hard gate: land the commits locally, report the state, and stop.

This skill has two modes:

- **Mode A — commit.** The user asks you to commit working/staged changes.
- **Mode B — verify.** The user asks whether an *already-made* commit captured everything. This includes when the user **committed it themselves** ("ฉัน commit ไปแล้ว", "ฉัน commit เองไปหมดละ / commit เองไปหมดแล้ว", "I already committed everything", "did I commit everything?", "is there anything left to fix?"). Read-only.

## Hard rules

- **Never push unless the user explicitly says to push** ("push", "push ขึ้น origin", "อัปขึ้น remote"). "Commit" alone is local-only. If unsure, do not push — ask.
- **Never amend, reset, or rebase away existing commits** unless explicitly asked. The user's history is theirs.
- **Never stage files you weren't asked about** when the change set is mixed and unrelated — see grouping below.
- **In Mode B, change nothing.** No staging, no committing, no edits. Inspect and report only.
- **Answer in the user's language.** Thai trigger ("commit ให้หน่อย", "คอมมิตไปแล้วนะ", "ฉัน commit ไปแล้ว", "ฉัน commit เองไปหมดละ") → reply in Thai.

## Mode A — commit

### 1. Inspect before committing

```bash
git status --short --branch      # branch, ahead/behind, staged + untracked
git diff && git diff --staged    # what actually changed
git log --oneline -10            # commit-message style + conventions to mimic
```

- Note the current branch and the **ahead/behind count** — this is what you report at the end.
- Read recent `git log` to learn the message convention (prefix style, language, scope) and whether the repo has patterns like committing skills/docs separately from feature work.
- Check workspace instructions (`AGENTS.md` / `CLAUDE.md`) for repo-specific commit rules (e.g. a mandatory `Release_Note.md` update on every change).

### 2. Honour repo conventions

Whatever the repo already does, do it:

- **Release notes / changelog**: if the workspace requires updating a `Release_Note.md` (or similar) on every change, make that edit *before* committing and include it in the relevant commit.
- **Separate concerns into separate commits**: if the repo historically commits scaffolding, skills, or docs as their own commit, do the same. Don't fold an unrelated untracked directory into the feature commit.
- **Message format**: match the existing prefix style (`feat:`, `fix:`, etc.) and the language the log uses.

### 3. Group into logical commits

- One concern per commit. A feature change and a stray new skill directory are **two** commits.
- Commit the coherent change set; leave clearly unrelated files out unless the user included them.
- Stage by explicit path (`git add <paths>`) rather than blanket `git add -A` when the tree is mixed.

### 3b. Cheap read-only verification before you commit

The full test/build gate (`cargo clippy`, `cargo test`, `npm test`, …) may be too expensive or slow to run here — especially on a RAM- or time-constrained machine, or when the user didn't ask for it. You can still gather *read-only* evidence that the change is safe, at zero cost:

- **Deletion/search-heavy change?** Grep the codebase (or `search_code`) for references to every symbol the commit **removes or renames**. Zero results = no dangling caller, which means the change cannot fail to compile on that account. This single check is often the strongest cheap evidence for a delete commit.
- **Rename?** Search for the old name to confirm no stragglers.
- **Config/flag removal?** Grep for the flag string across the tree.

Say explicitly that this is **read-only evidence, not a build**. It narrows the risk but does not substitute for the real gate — see the verification-gap note in the report step and in Gotchas.

### 4. Commit — do not push

```bash
git commit -m "<prefix>: <summary>"
```

For multiple commits, repeat. After the last commit, confirm the tree is clean and the branch is ahead of origin by the expected count — nothing pushed.

### 5. Report

Give a short, scannable summary:

1. **Branch + ahead count** (`git status` line, e.g. `## Rust...origin/Rust [ahead 6]`) and an explicit statement that **nothing was pushed**; the remote ref is unchanged.
2. **Per-commit table**: hash, message, and files changed with `+/-` counts.
3. **What the commit contains** in one or two lines of plain description per commit.
4. If you split an untracked directory into its own commit, say why, and offer the undo: `git reset --soft HEAD~1` (files return to staged, nothing lost).
5. **Flag verification gaps honestly**: if fmt/clippy/tests were *not* run against exactly what was just committed, say so — don't imply the code is verified. If you did the cheap read-only check from step 3b (e.g. zero references to a deleted symbol), present *that* as the evidence you have, and clearly separate it from the build/test gate you did not run.
6. **Name what's still untracked**, if anything (`?? <path>` lines), so the user knows the tree isn't fully recorded — and offer to commit it separately rather than sweeping it in.

## Mode B — confirm a just-made commit is complete

**Read-only.** Its job is to answer two *different* questions the user lumps into one sentence:

1. **Completeness** — is anything still uncommitted?
2. **Verification** — is the committed work actually verified?

They can disagree. A clean tree can be true while the work is still unverified, so answer both, separately. Never say "ใช่ ไม่มีอะไรต้องแก้แล้ว" on the strength of a clean tree alone.

There is a third, quieter question the user usually doesn't ask out loud but cares about: **did the commit capture what I *thought* it did — no more and no less?** A commit can be complete (nothing left uncommitted) and still have *swallowed a file the user didn't expect*. Reconcile the committed file list against the user's belief; see B2.

### B1. Inspect

```bash
git status --porcelain --untracked-files=all   # the ?? / M lines are the whole answer
git log --oneline -5
git show --stat HEAD            # what the last commit actually captured
git show -s --format='%H%n%an%n%ad%n%s' HEAD
git show --name-status HEAD     # per-file A/M/D for reconciling against expectations
```

- **Use `--untracked-files=all` (a.k.a. `-uall`), not bare `git status --short`.** Plain `git status` collapses each untracked directory into a single `?? dir/` entry, which can hide whether individual files inside it were recorded. `-uall` expands to every untracked file, so an empty untracked section is real proof rather than a collapsed summary. (The report can print a labelled `---UNTRACKED---` section beneath the status output so the emptiness is visible.)
- Comparing `--porcelain` output against a prior round's is the cleanest way to show that a previously-untracked directory (e.g. a new skill dir) has since been captured by the user's own commit.
- **Also `git show --name-status HEAD`** to get the *count* and *list* of files the commit touched, so you can reconcile it against what the user believed they committed (see B2).

### B2. Classify any leftovers — "leftover" is not the same as "bug"

Read the porcelain codes literally:

- **No `M` / `A` / `D` lines** → all tracked changes are committed. Question 1 is answered **yes**.
- **`?? <path>` untracked** → *not* something to fix; something never recorded. Repeatedly this is a **new skill / doc / scaffold directory** (e.g. `.agents/skills/<name>/SKILL.md`). Say plainly it is untracked, not modified, and offer to commit it as its own commit per repo convention.
- **`M <path>` (unstaged) or `M  <path>` (staged)** → a genuine uncommitted edit. This *is* something left. Do **not** say "nothing to fix."
- **`git show --stat HEAD`** → confirm the committed file list matches the change set the user believed they committed (compare against the change set from an earlier round if you have it). A stat that matches the expected files is the strongest single confirmation.
- **When the user says they committed it themselves**, also confirm HEAD advanced: `git log --oneline -5` should show their new commit on top, and the previously-untracked path should now appear in its `--stat`. HEAD-moved + untracked-now-empty together is the complete "yes, it's all in" answer.

#### Reconcile over-inclusion: the commit swept in a file the user didn't expect

This is the subtle failure a plain "tree is clean" check misses. Do it whenever you have the user's *expected* file list (their own summary, your prior round's list, or their stated belief of "I committed N files"):

1. **Compare counts and names.** `git show --name-status HEAD` vs the expected set. Do the counts match? Are all expected files present?
2. **If the commit has MORE files than expected, do not hand-wave it as "all good."** Name the extra file(s) explicitly. Then read the extra file's diff (`git show HEAD -- <path>` or `git diff HEAD~1 HEAD -- <path>`) and judge:
   - **Same coherent change set** (e.g. it's the agent→TUI bridge wiring touched by the same feature, uses the same new types, edits parts of the same subsystem) → it *belongs*; say so, and note that the earlier summary simply omitted it. Flag that you hadn't listed it before rather than pretending it was expected.
   - **Unrelated to the change set** (a stray file, an accidental `git add -A` pickup, an unrelated doc) → surface it as a real concern and offer the fix: `git reset --soft HEAD~1` then re-commit without it, or a follow-up commit that removes it. Only on explicit instruction.
3. **If the commit has FEWER files than expected**, that's the mirror problem — a file the user thought they committed isn't in the commit. Check `git status` and `git log` to see whether it's still uncommitted, in a *different* commit, or never staged. Name it.

A count mismatch is signal, not noise: it means the user's mental model of their own commit is off, and telling them precisely how is more useful than a green checkmark.

### B3. Report the two answers separately

1. **Completeness verdict** — "working tree clean of tracked changes: `<hash>` captured all N files" **or** "1 genuine uncommitted edit remains: `<path>`".
2. **Name the leftover** — if only `??` files remain, state they are untracked (never committed), name them, and note it's a doc/skill addition, not a broken change.
3. **Reconciliation** — state the committed file count and whether it matches what the user expected. If it doesn't, name the delta and your judgment (belongs / stray). Don't let an unexpected file pass silently.
4. **Remote state** — the `ahead N` count and an explicit "nothing pushed." Never imply pushed. Optionally list the pending local commits (`git log --oneline origin/<branch>..HEAD`) so the user sees what's queued.
5. **Verification gap** — say exactly which gates ran against the committed content and which did not. `cargo fmt --check` passing ≠ `clippy`/`test` passing. Do not imply green CI.
6. **Options**, short and scannable: push the queue (only on explicit instruction) / run the missing verification / view the combined diff (`git diff origin/<branch>..HEAD --stat`). Recommend the low-risk one; perform none of them without instruction.

If the user wants to un-commit, `git reset --soft HEAD~1` is safe — files return to staged, nothing lost.

## Gotchas

- **"Commit" ≠ "push".** The single most important distinction. Report `ahead N` and stop; pushing is a separate, explicitly-authorized action.
- **"Nothing to commit" ≠ "nothing to fix" ≠ "verified".** The user asking "ไม่มีอะไรให้แก้แล้วใช่ป่ะ" — or "ฉัน commit ไปแล้ว" — usually means *"is the work done?"* A clean tree answers only the completeness half. State the verification gap in the same breath, or the user assumes green CI.
- **A clean tree doesn't mean the commit is *right*.** The commit can have swallowed an extra file (over-inclusion) or dropped one the user expected (under-inclusion). Reconcile the `--name-status` list against the user's expectation; a count mismatch is signal. A stray file folded in by a broad `git add` is invisible to a bare `git status` check.
- **Over-inclusion needs a judgment, not a shrug.** When the commit has more files than the user's summary listed, read the extra file's diff and decide: same coherent change set (belongs — and you missed listing it before) vs unrelated (real concern). Never just say "all good" without naming the delta.
- **Untracked `??` files are leftovers, not bugs.** New skill/doc directories land here constantly (and in this repo are committed separately). Don't lump them into "something you still have to fix" — name them, say they're untracked, and offer the separate commit.
- **Read the porcelain codes literally.** `M ` (staged) and ` M` (unstaged) mean *there is an uncommitted change*; `??` means *never recorded*. They lead to different answers.
- **Bare `git status` hides untracked files inside directories.** Use `--untracked-files=all` when you're proving the tree is fully recorded — otherwise a collapsed `?? dir/` can mask whether the individual files were captured.
- **Mixed working tree.** Untracked scaffolding (skills, generated dirs) sitting next to real feature edits is common. Split it; don't silently sweep it into the feature commit.
- **Deletion commits without a build.** A commit that *removes* a function is the easiest to verify cheaply: grep/`search_code` for references to the removed symbol. Zero hits means no dangling caller and the change can't break the build on that account. Present this as read-only evidence, explicitly *not* a `clippy`/`test` run.
- **CI gates you didn't run.** A previously-passed `fmt --check` does not mean `clippy`/`test` passed on this change set. State which checks ran and which didn't rather than claiming green. When the machine is RAM-constrained, say so and run gates one at a time (or hand the commands to the user) rather than skipping silently.
- **Faking status.** Don't report an accurate ahead-count without running `git status` — compute it, don't assume.
- **Rewriting history.** `reset --soft HEAD~1` to un-commit is fine and safe (staged files survive); `--hard`, `rebase`, or `amend` on shared history are not — only do those on explicit instruction.
