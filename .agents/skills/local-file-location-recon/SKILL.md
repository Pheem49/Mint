---
name: local-file-location-recon
description: Locate a file or folder on the local machine (album, photo set, download, project dir) and report where it actually lives plus its structure — search the usual roots case-insensitively, handle mixed-script names, and don't assume standard folders. Trigger when the user says 'where is my X / หาไฟล์/โฟลเดอร์ X ไม่เจอ / อยู่ในเครื่องตรงไหน'.
revisions: 1
---

# Local File Location Recon

Answer "where on this machine is X?" with real evidence from the filesystem, then describe **what's actually in** it (count, size, subfolders). The single most common surprise: **the thing is not where the user (or you) assumes it should be** — albums land in `~/Downloads`, not `~/Pictures`; source trees land wherever they were cloned. Search the grass-catcher directories, don't trust the tidy hierarchy.

This is **read-only** reconnaissance unless the user asks you to move/open something.

## Core stance

- **Don't assume standard locations.** "An album of photos" is not automatically in `~/Pictures`. Check `~/Downloads` first — casual downloads, browser saves, and unzipped archives almost always land there and stay there.
- **Search case-insensitively.** `machi` vs `Machi` vs `MACHI` are different strings to `find`/`ls` by default. Use `-iname`, not `-name`.
- **Expect mixed-script names.** Names from Thai/Japanese/Chinese sources mix scripts (`Machi馬吉`, `โฟลเดอร์`, romanized+kanji). Match on the romanized substring, not the whole string, and don't require an exact spelling.
- **Report the structure, not just the path.** "Found it" is half an answer — the user wants to know how many files, what format, how big, whether there are subfolders.
- **Answer in the user's language.** Thai trigger (`หา X ไม่เจอ`, `X อยู่ไหน`, `อยู่ในเครื่องตรงไหน`) → reply in Thai.

## Workflow

### 1. Search the likely roots, case-insensitively

Fast first pass — glob the usual suspects for the name fragment:

```bash
ls -la ~/Downloads ~/Pictures ~/Documents ~/Videos ~/Desktop ~/Music 2>/dev/null | grep -i '<term>'
```

If that misses, widen to a bounded `find` from home:

```bash
find ~ -maxdepth 4 -iname '*<term>*' 2>/dev/null
```

- `-maxdepth 4` keeps it fast and avoids descending into `node_modules`, `.cache`, deep project trees. Raise it only if the shallow search misses.
- Drop `-type d` for folders, `-type f` for files, or omit for both.
- **If `plocate`/`locate` is installed, use it first — it's near-instant**: `locate -i '<term>' | grep "$HOME"`. Fall back to `find` when the index doesn't exist or is stale.

### 2. Disambiguate when there are several hits

If multiple matches come back, list them and pick by context (the user said "album"/`รูป`, so a folder full of `.jpg` fits; a random `.log` doesn't). State which hit you chose and why rather than guessing silently.

### 3. Describe what's inside

Once found, gather the details that make the report useful:

```bash
ls -la ~/path/to/thing            # top-level contents, note subfolders
find ~/path/to/thing -maxdepth 1 -type f | wc -l   # file count at top level
find ~/path/to/thing -type f | wc -l               # total file count (recursive)
du -sh ~/path/to/thing            # total size
```

Note the file **extension mix** and whether there's a **subfolder** (a `compressed/` set alongside originals is common when the user keeps a downsized copy). Flag subfolders — they change which one the user should open.

### 4. Report and offer next steps

Lead with the exact path, show the structure, then offer concrete actions — but **don't perform destructive or moving operations unasked**.

## Output shape

1. **Verdict line + exact path up front** — "didn't find it in Pictures; it's in `~/Downloads/Machi`". A one-line `xdg-open "<path>"` the user can paste is a nice touch.
2. **Structure block** — a small tree or indented listing of the folder and its subfolders.
3. **Details table** — name, count of files, extensions, total size, location. Label facts as read from the machine.
4. **Note any subfolder** (e.g. a `compressed/` lighter set) and which one to use for a quick/lightweight view.
5. **Observation** — call out when it's in an unexpected place ("it's sitting in `Downloads`, never moved into `Pictures`").
6. **A/B/C next steps**, in the user's language — e.g. (A) move it into `~/Pictures/…`, (B) open the lighter subfolder now, (C) verify the set is complete (numbers 01–42 present, no dupes). Do none of these without instruction.

## Gotchas

- **`~/Downloads` is the grass-catcher.** Browser downloads, unzipped archives, and saved albums live there long after the user thinks they filed them. Check it first, not last.
- **`find` is case-sensitive by default** — always `-iname`. A "missing" file is often just a capital-letter difference.
- **Mixed-script names break exact matching.** Match on a romanized fragment (`-i machi`), not the full multi-script string. Same for Thai filenames — match a distinctive substring.
- **Unbounded `find ~` is slow and noisy.** Bound with `-maxdepth`; if you need deeper, `locate`/`plocate` beats it. Don't let a search walk `.cache`, `node_modules`, or `target/`.
- **Don't assume one location.** A name can appear in several roots; if so, list them all and let the user choose rather than reporting the first hit as "the" answer.
- **A folder is more than its path.** Without count/size/subfolders the user still can't tell if it's the right set or a partial one.
- **Read-only means read-only.** Searching is fine; moving, deleting, or opening files should be offered, not done — mirror the "don't `rm -rf` on the user's behalf" discipline of the other machine-recon skills.
- **Cross-reference when the user pivots.** "Can I add disk?" → `hardware-upgrade-check`; "is my disk full?" → `cargo-clean-decision`; "what am I running?" → `machine-session-recon`.
