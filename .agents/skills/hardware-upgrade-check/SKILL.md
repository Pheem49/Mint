---
name: hardware-upgrade-check
description: Assess the local machine's RAM — answer 'how much RAM do I have?' (installed vs usable) and whether it can be upgraded: gather live specs without sudo, cross-reference the vendor's official spec sheet, reconcile the classic soldered-vs-socket conflict, and deliver a definitive verdict with the exact upgrade part. Trigger when the user asks 'how much RAM do I have?', 'can I add more RAM?', 'check my machine spec', or in Thai 'ในเครื่องมีแรมเท่าไหร่ / ดูสเปคเครื่องหน่อย / ใส่แรมเพิ่มได้มั้ย'.
revisions: 2
---

# Hardware Upgrade Check

Answer "how much RAM do I have / can I upgrade this machine?" with real evidence, not a guess. This is a multi-source investigation: live system data, the vendor's official spec sheet, and a reconciliation step, because the two **often contradict each other**.

Two distinct questions live here, and they need different evidence:

- **"How much RAM is in my machine?"** — a *counting* question. Answer it from the machine alone (installed vs usable), no web needed.
- **"Can I add more?"** — an *investigation* question. Needs the vendor spec sheet and the reconciliation below.

Always answer the count first even when the user only asks the upgrade question — the count is where the surprises hide (installed 8 GB, usable 5.6 GiB).

## Workflow

### 1. Answer the count: installed vs usable

Report **two numbers**, not one, because they diverge and the gap is the story:

- **Installed** — the physical module size (e.g. 8 GiB), read from DMI/`inxi`.
- **Usable** — what the kernel actually gets, `MemTotal` from `/proc/meminfo` (`free -h` rounds it, e.g. `5.6Gi`).

```bash
free -h
awk '/MemTotal|MemAvailable|SwapTotal|SwapFree/ {print}' /proc/meminfo
```

- Normal reserved overhead is ~0.3–0.5 GiB (firmware, kernel, hardware reservations).
- A **gap much larger than that** (e.g. 8 GB installed → 5.62 GiB usable, ~2.4 GiB missing) is usually an **iGPU sharing system RAM** via a large UMA frame buffer, or a BIOS setting. Name it as the likely cause and suggest the BIOS → Graphics / UMA Frame Buffer Size knob (don't tell them to change it unprompted).
- **Always include the pressure snapshot.** If `used` is near `total` and **swap is in use**, say so — it explains "the machine is slow when I run builds," and it's the real reason the user asked. Cross-reference `machine-session-recon` (what's eating RAM) and `cargo-clean-decision`.

### 2. Gather specs without sudo first

`sudo dmidecode` is the gold standard but needs a password you may not have. **Get most of the picture read-only first**, then hand the `sudo` commands to the user as the definitive check.

No-sudo sources that genuinely work:

```bash
inxi -m                       # memory: Array capacity, # slots, # modules, per-slot type/speed, empty slots
lscpu                         # CPU
lspci | grep -iE 'vga|3d'     # GPU
lsblk                         # disk
systemd-detect-virt           # bare metal (none) vs container/VM (kvm, docker, …)
```

**Read the machine identity straight out of sysfs — no sudo needed:**

```bash
cat /sys/devices/virtual/dmi/id/sys_vendor        # e.g. LENOVO
cat /sys/devices/virtual/dmi/id/board_name        # e.g. LNVNB161216
cat /sys/devices/virtual/dmi/id/product_name      # e.g. 82XM  ← the model code to cross-reference
cat /sys/devices/virtual/dmi/id/product_version
```

These four files are the read-only substitute for `sudo dmidecode -t 1 / -t 2` and give the exact product code you'll search the vendor site with. Use them whenever sudo is unavailable.

With sudo, `dmidecode -t 17` (memory devices), `-t 16` (max capacity), `-t 0`, `-t 1` (system), `-t 2` (baseboard) round it out.

### 3. Read the memory configuration carefully — this is where the answer hides

`dmidecode -t 17` / `inxi -m` print one block per memory device/slot. Key fields:

- `Form Factor: SODIMM` → physically removable stick. `Soldered`/`Row of chips`/`Unknown` → likely not removable.
- `Size: 8 GB` vs `Size: No Module Installed` → populated vs **empty slot** (note the `Locator`, e.g. `ChannelA-DIMM0` / `ChannelB-DIMM0`, to say which channel is free).
- `inxi -m` summarizes the array in one line: `Array-1: capacity: 8 GiB slots: 2 modules: 1` — slots > modules means a free socket is plausible; note it, then confirm below.
- `dmidecode -t 0` or `-t 16` (physical memory array) reports **Maximum Capacity** — the BIOS/SMBIOS cap. A low cap is a real constraint even if a socket exists.
- Memory **type/speed** (`DDR4`, `3200 MT/s`) is itself a clue: DDR4 SODIMM at 3200 points to a socketed stick; many soldered low-power laptops use LPDDR5 instead (see the gotcha in step 5).

### 4. Cross-reference the vendor's official spec — never trust DMI alone

- Lenovo: find the PSREF page for the exact model (e.g. `psref.lenovo.com`, or search `"<model>" memory soldered psref`). PSREF explicitly says "Memory soldered to systemboard, no slots" when that is the case.
- Dell/HP/ASUS/etc.: their published spec sheets state soldered vs SO-DIMM slots the same way.
- Community (Reddit/forums) and third-party sites (e.g. Technize, laptopmedia) only as a tie-breaker, and **label them as community/third-party, not a spec.**

### 5. Reconcile the evidence — expect this exact conflict

Classic gotcha: **live DMI reports 2 slots while the vendor says soldered/no slots.** Both can be partly true:

- Some models ship **one soldered chip + one real SO-DIMM socket** — DMI lists both banks, so an "empty slot" can be real.
- Others are fully soldered and the second DMI slot is a phantom entry.
- BIOS max capacity may cap the machine below what the socket could physically accept.

**Model codes are not unique.** A single code (e.g. `82XM` = IdeaPad Slim 3 family) maps to **many sub-models**, and the memory config can differ sub-model to sub-model (one sub-model soldered LPDDR5, another socketed DDR4). A web page for one sub-model is *not* a spec for the user's exact unit. Two extra tells when the machine and the web disagree:

- If the web claims the family is soldered **LPDDR5** but DMI reads **DDR4-3200 SODIMM**, the web page is probably about a different sub-model — the machine wins.
- DMI reporting a `DIMM` slot with `No Module Installed` alongside a real speed/type is strong evidence for a physical socket.

When sources conflict: say so explicitly, present both sides **with their sources** ("from machine DMI/inxi" vs "from Lenovo PSREF / Technize"), and give the definitive checks below rather than claiming an answer you can't establish. **A "run these two commands and you'll know 100%" answer is the honest, more useful one.**

### 6. Definitive verification — hand these to the user as next steps

- `sudo dmidecode -t 17` — if a `No Module Installed` block also says `Form Factor: SODIMM`, a real free socket exists.
- `sudo dmidecode -t 16` — Maximum Capacity (BIOS cap).
- `sudo dmidecode -t 1` — the full Version/Product Name to match against the vendor PSREF.
- Physical: open the back panel and look for an empty SO-DIMM slot. On many consumer laptops (e.g. IdeaPad Slim 3) this is 6–8 screws and easy — say so when it's true; recommend a shop only for machines that are hard to open (glued/ultrabooks).

### 7. If upgradeable, give the exact module

Derive the spec from the installed stick (via `dmidecode -t 17` / `inxi -m` speed+type): generation (DDR4/DDR5), speed (`3200` = PC4-25600), form factor SO-DIMM, non-ECC. Recommend matching the installed module's speed for dual-channel pairing. Give a typical retail price in the user's currency and note capacity caps / BIOS caveats.

## Output shape

1. **Short verdict line up front**, in the user's terms — e.g. "8 GB DDR4-3200, one stick, ~5.6 GiB usable." For a pure 'how much RAM' question, lead with the number.
2. **Count table**: installed vs usable vs currently used vs swap, each with its source (from machine: `inxi`/`/proc/meminfo`/`free`).
3. **Full spec table** (component → detail), clearly labeled as read from the live machine (CPU, RAM with usable-vs-reserved if iGPU shares it, SSD, WiFi, battery, OS, virtual-vs-bare-metal).
4. **Verdict** on upgradeability: possible / not possible / indeterminate-due-to-conflict.
5. **The two evidence sides** when they conflict, each with its source.
6. **Concrete next steps** the user can run themselves — especially the `sudo` commands you could not run.
7. **If buying**: exact module spec + typical cost + caveats.

## Rules

- **Never invent specs.** Every fact carries a source label: "from machine (dmidecode/inxi/lscpu/sysfs)", "from vendor spec sheet (PSREF)", "from community/third-party web".
- **Report installed *and* usable.** One number hides the iGPU/UMA gap and the memory pressure that made the user ask.
- **Distinguish verified from unverified.** "Spec sheet says no slots" is not the same as "I opened it and saw no slot". Keep the confidence level visible.
- **Can't run sudo?** Hand over the exact command rather than skipping the definitive check. Get the model code from `/sys/devices/virtual/dmi/id/` in the meantime.
- **A model code is not a spec.** Cross-reference the exact sub-model; a family-level web page can be wrong for this unit.
- **Answer in the user's language.** Triggers may be Thai (`ในเครื่องมีแรมเท่าไหร่`, `ดูสเปคเครื่องหน่อย`, `ใส่แรมเพิ่มได้มั้ย`) — respond in Thai if the user wrote Thai.
- Don't manufacture certainty when two authoritative sources disagree; a "check these two things and you'll know 100%" answer is the honest and more useful one.
- **Read-only means read-only.** Don't change BIOS settings or run `dmidecode`-mutating actions unasked; suggest, then let the user act.
