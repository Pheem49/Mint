---
name: video-frame-extraction
description: Give ready-to-run ffmpeg (and GUI) commands for extracting still frames / contact sheets from recorded video — at a timestamp, every N seconds/frames, or per scene change — plus the gotchas (mkdir first, -ss before vs after -i, -fps_mode vfr, disk cost of PNG). Trigger when the user asks to 'extract frames/photos from a video', make thumbnails or a storyboard, or 'แยกรูปภาพจากวิดีโอ'.
revisions: 1
---

# Video Frame Extraction

Answer "give me a command to pull images out of a recorded video" with a small menu of **copy-pasteable ffmpeg commands**, ordered by what the user actually wants (one frame vs a series vs per-scene), plus the traps that make the naive command fail. Confirm `ffmpeg` exists first, probe the video, then hand over commands rather than running a destructive/huge extraction unasked.

## Step 0 — Verify the tool and probe the input

```bash
ffmpeg -version | head -1        # confirm ffmpeg is installed at all
ffprobe -v error -select_streams v:0 \
  -show_entries stream=codec_name,width,height,r_frame_rate,nb_frames \
  -show_entries format=duration,size \
  -of default=noprint_wrappers=1 input.mp4
```

- Duration (seconds) + fps + resolution are what let you compute the right interval for "give me N stills across the clip".
- Duration only: `ffprobe -v error -show_entries format=duration -of csv=p=0 input.mp4`
- If the user's video path isn't given, or the folder is empty, say so — don't assume a file exists. Offer to locate recordings (`~/Videos`, OBS output, `~/Downloads`, SD card). Users can also drag a file into the terminal to get a real path.

## Step 1 — Pick the extraction shape

| Want | Command |
|---|---|
| **One frame at a timestamp (fast)** | `ffmpeg -ss 00:00:12 -i input.mp4 -frames:v 1 -q:v 2 shot_12s.jpg` |
| **One frame, frame-exact** | `ffmpeg -i input.mp4 -ss 12.500 -frames:v 1 -q:v 2 shot_12.5s.jpg` |
| **Every 1 s (PNG)** | `mkdir -p frames && ffmpeg -i input.mp4 -vf fps=1 frames/frame_%04d.png` |
| **Every 5 s (JPG)** | `mkdir -p frames && ffmpeg -i input.mp4 -vf fps=1/5 -q:v 2 frames/frame_%04d.jpg` |
| **Every N frames (e.g. 30)** | `ffmpeg -i input.mp4 -vf "select='not(mod(n\,30))'" -fps_mode vfr frames/frame_%04d.png` |
| **Exactly N stills across the clip** | Don't use a raw `fps=` — compute `interval = duration / N`, then `fps=1/<interval>` |
| **On scene change** | `ffmpeg -i input.mp4 -vf "select='gt(scene,0.3)',scale=640:-1" -fps_mode vfr -frame_pts 1 scene_%03d.jpg` |
| **Contact sheet / thumbnail grid** | `ffmpeg -i input.mp4 -vf "fps=1/10,scale=320:-1,tile=4x4" -frames:v 1 sheet_%02d.jpg` |
| **Every single frame** | `ffmpeg -i input.mp4 frames/frame_%05d.png` (huge — warn about disk) |

## Step 2 — Options worth knowing

| Need | Add |
|---|---|
| Resize / crop | `-vf "fps=1,scale=1280:-2"` — height `-2` keeps aspect ratio *and* stays even |
| JPG quality | `-q:v 2` (1 = best, 31 = worst) |
| WebP (space-saving) | `-c:v libwebp -q:v 80` |
| Frame number in filename | `-frame_pts 1` (gives frame index, not wall-clock seconds) |
| Overwrite existing files | `-y` (otherwise ffmpeg prompts on stdin and a loop **hangs**) |
| Quiet output | `-hide_banner -loglevel error` placed **before** `-i` |
| Mobile VFR clips | an `fps=` filter normalizes timing automatically — nothing extra needed |

## Step 3 — Batch many clips

```bash
for f in ~/Videos/rec/*.mp4; do
  n=$(basename "$f" .mp4)
  mkdir -p "frames/$n"
  ffmpeg -hide_banner -loglevel error -y -i "$f" -vf fps=1/2 -q:v 2 "frames/$n/%04d.jpg"
  echo "done: $n"
done
```

Produces `frames/<clip-name>/0001.jpg …` with one folder per input automatically.

## Step 4 — Non-CLI alternatives (offer these)

- **VLC**: Video → Snapshot, or `Shift+S` — single frame, interactive.
- **LosslessCut / Shotcut** — scrub a timeline and pick points visually.
- **ImageMagick** `magick montage` — build a contact sheet from already-extracted frames.
- If the environment has built-in video tools (e.g. a `video_filmstrip` / `video_waveform` tool), offer those to preview *where* the wanted frames are before doing the real extraction. Note that a single-frame export still needs ffmpeg unless such a tool exists.

## Gotchas

- **Always `mkdir -p frames` before an extraction loop.** ffmpeg does **not** create the output directory; it errors `No such file or directory` on the first frame.
- **`-ss` before `-i` = fast but keyframe-aligned** (can be off by a few frames); **after `-i` = frame-exact but decodes from the start.** Pick one, or two-pass if you need both. State the tradeoff — it's the most common surprise.
- **ffmpeg 6.x deprecates `-vsync vfr`** → use **`-fps_mode vfr`**. Old tutorials still show `-vsync`.
- **`-y` matters inside loops**, or ffmpeg blocks on an overwrite prompt forever.
- **PNG is lossless but huge — disk is the real cost.** Every-frame extraction of a long 1080p clip can be tens of GB. If free space is tight (`df -h .`), recommend **JPG `-q:v 3`** or downscale with `-vf scale=1280:-2` instead of PNG. Reuse the disk-headroom reasoning from the `cargo-clean-decision` skill if space is a concern.
- **"Exactly N stills" is not `fps=1/N`** — compute the interval from the probed duration first.
- **Frame number ≠ seconds.** `-frame_pts 1` puts the frame index in the name, not a wall-clock time.
- **Don't kick off a giant extraction unasked.** Hand over the commands, or confirm scope (interval, format, count) before running — mirror the "don't delete on the user's behalf" discipline.
- **Answer in the user's language.** Thai trigger ("แยกรูปภาพจากวิดีโอ") → reply in Thai.

## Output shape

1. **Tool check** — is ffmpeg/ffprobe installed, and which version (drives `-fps_mode` vs `-vsync`).
2. **Probe result** — duration/fps/resolution if a path was given; otherwise note the file wasn't found and ask for it.
3. **Step-by-step command menu** — probe first, then the extraction shapes, then options.
4. **Traps called out** — mkdir, `-ss` placement, `-fps_mode vfr`, disk cost.
5. **GUI/tool alternatives** for users who don't want the terminal.
6. **Offer A/B/C next steps** — give me the video path / tell me interval+format and I'll compose one command / let me find the recordings first.