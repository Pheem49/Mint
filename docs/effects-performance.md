# Background and orb performance

## Behavior

Desktop and Web share **Settings → Theme → Reduce effects**. Either that setting or the OS reduced motion preference freezes chat glows and the orb. Blur radius, antialiasing, shader precision and the existing DPR cap remain unchanged.

The orb renders on visual changes, including size and hue. It suspends its frame loop while offscreen or the document is hidden, and resumes when visible. Paused/error Live calls and the recovery screen use a static orb. Reduced motion preserves standalone voice detection callbacks through a 100 ms sampling timer without submitting identical GPU frames. Microphone and Live audio ownership are unchanged. CLI/TUI has no equivalent graphical effects; Rust’s flattened config preserves the setting.

## Measurement (2026-10-10)

Measured with hardware accelerated headless Chrome, ANGLE OpenGL, **AMD Radeon Graphics (radeonsi renoir ACO)**. A 300 × 300 CSS pixel orb and the real chat glow CSS were mounted in the fixture. Instrumented `drawArrays` and `EXT_disjoint_timer_query_webgl2`; no GPU disjoint samples occurred. Each settled mode was sampled for five seconds.

| Mode | Original orb draws / 5 s | Updated orb draws / 5 s | Updated background animation |
| --- | ---: | ---: | --- |
| Normal | 300 | 301 | floatGlowOne |
| OS reduced motion | 234 | 0 | none |
| Reduce effects | 235 (setting did not exist) | 0 | none |

Final updated normal-mode GPU draw time: median **0.06688 ms**, p95 **0.47934 ms**, total **42.71126 ms** over 301 samples. Run-to-run timing varied; these are individual orb draw measurements, not whole-frame or whole-app GPU utilization. The original normal run measured median 0.05428 ms / p95 0.4824 ms. No normal-mode speedup or image-quality reduction is claimed.

Additional measured behavior: offscreen and paused each produced zero draws over one second; returning onscreen resumed approximately 60 draws/s. A hue change in reduced effects produced one draw, then stopped. After resizing, the settled orb again produced zero repeated draws.

**Limits:** This is a Chrome fixture, not native Tauri/WebKit profiling. Timer queries isolate the orb draw; they do not measure CSS blur/compositor cost, GPU power or memory. Keep the current blur quality until native compositor measurements support a change.

## Reproduce

Requires Google Chrome and Node with the global WebSocket API. Start the Vite server, then run the measurement from another terminal:

```sh
npm run dev:web
node src/bin/measure-effects.mjs
```

The driver opens a temporary Chrome profile, requests hardware rendering and prints the actual renderer plus timer availability. Software rendering results must not be treated as hardware GPU measurements. The optional first argument overrides the fixture URL. It closes its own browser and removes its temporary profile on completion.
