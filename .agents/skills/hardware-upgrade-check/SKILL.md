---
name: hardware-upgrade-check
description: Assess whether the local machine's RAM (or other hardware) can be upgraded — gather live specs, cross-reference the vendor's official spec sheet, reconcile the classic soldered-vs-socket conflict, and deliver a definitive verdict with the exact upgrade part. Trigger when the user asks 'can I add more RAM?', 'check my machine spec', or in Thai 'ดูสเปคเครื่องหน่อย / ใส่แรมเพิ่มได้มั้ย'.
revisions: 1
---

# Hardware Upgrade Check

Answer "can I add more RAM / upgrade this machine?" with real evidence, not a guess. This is a multi-source investigation: live system data, the vendor's official spec sheet, and a reconciliation step, because the two **often contradict each other**.

## Workflow

### 1. Gather specs from the live machine

Pull real data. Never invent specs.

- CPU: `lscpu`
- GPU: `lspci | grep -iE 'vga|3d'`
- Memory: `sudo dmidecode -t 17` (preferred — needs sudo; if unavailable use `inxi -m` or `inxi -Fxz` and say sudo is needed for the definitive check)
- Disk: `lsblk`
- Model: `sudo dmidecode -t 1` (System Information) / `-t 2` (Baseboard) — the exact model string is what you cross-reference later
- `inxi -Fxz` gives a one-shot summary when installed

### 2. Read the memory configuration carefully — this is where the answer hides

`dmidecode -t 17` prints one block per memory device/slot. Key fields:

- `Form Factor: SODIMM` → physically removable stick. `Soldered`/`Unknown` → likely not removable.
- `Size: 8 GB` vs `Size: No Module Installed` → populated vs **empty slot** (note the `Locator`, e.g. `ChannelA-DIMM0` / `ChannelB-DIMM0`, to say which channel is free).
- `dmidecode -t 0` or `-t 16` (physical memory array) reports **Maximum Capacity** — the BIOS/SMBIOS cap. A low cap is a real constraint even if a socket exists.

### 3. Cross-reference the vendor's official spec — never trust DMI alone

- Lenovo: find the PSREF page for the exact model (e.g. `psref.lenovo.com`, or search `"<model>" memory soldered psref`). PSREF explicitly says "Memory soldered to systemboard, no slots" when that is the case.
- Dell/HP/ASUS/etc.: their published spec sheets state soldered vs SO-DIMM slots the same way.
- Community (Reddit/forums) only as a tie-breaker, and label it as community info, not a spec.

### 4. Reconcile the evidence — expect this exact conflict

Classic gotcha: **live DMI reports 2 slots while the vendor says soldered/no slots.** Both can be partly true:

- Some models ship **one soldered chip + one real SO-DIMM socket** — DMI lists both banks, so an "empty slot" can be real.
- Others are fully soldered and the second DMI slot is a phantom entry.
- BIOS max capacity may cap the machine below what the socket could physically accept.

When sources conflict: say so explicitly, present both sides **with their sources** ("from machine DMI" vs "from Lenovo PSREF"), and give the definitive checks below rather than claiming an answer you can't establish.

### 5. Definitive verification — hand these to the user as next steps

- `sudo dmidecode -t 17` — if a `No Module Installed` block also says `Form Factor: SODIMM`, a real free socket exists.
- Physical: open the back panel and look for an empty SO-DIMM slot. On many consumer laptops (e.g. IdeaPad Slim 3) this is 6–8 screws and easy — say so when it's true; recommend a shop only for machines that are hard to open (glued/ultrabooks).

### 6. If upgradeable, give the exact module

Derive the spec from the installed stick (via `dmidecode -t 17` speed/type): generation (DDR4/DDR5), speed (`3200` = PC4-25600), form factor SO-DIMM, non-ECC. Recommend matching the installed module's speed for dual-channel pairing. Give a typical retail price in the user's currency and note capacity caps/BiOS caveats.

## Output shape

1. **Spec table** (component → detail), clearly labeled as read from the live machine (CPU, RAM with usable-vs-reserved if iGPU shares it, SSD, WiFi, battery, OS).
2. **Verdict**: possible / not possible / indeterminate-due-to-conflict.
3. **The two evidence sides** when they conflict, each with its source.
4. **Concrete next steps** the user can run themselves — especially the `sudo` commands you could not run.
5. **If buying**: exact module spec + typical cost + caveats.

## Rules

- **Never invent specs.** Every fact carries a source label: "from machine (dmidecode/lscpu)", "from vendor spec sheet (PSREF)", "from community".
- **Distinguish verified from unverified.** "Spec sheet says no slots" is not the same as "I opened it and saw no slot". Keep the confidence level visible.
- **Can't run sudo?** Hand over the exact command rather than skipping the definitive check.
- **Answer in the user's language.** Triggers may be Thai (`ดูสเปคเครื่องหน่อย`, `ใส่แรมเพิ่มได้มั้ย`) — respond in Thai if the user wrote Thai.
- Don't manufacture certainty when two authoritative sources disagree; a "check these two things and you'll know 100%" answer is the honest and more useful one.
