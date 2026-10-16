# Shared UI components

Mint already uses React and TypeScript. Its two Vite applications previously used
plain CSS and separate `@/` roots. Reusable UI lives in
`src/renderer/shared/components/ui`, rather than a new top-level application root.
Keeping this `components/ui` folder gives the shadcn CLI a stable destination and
prevents Desktop and Web from acquiring different copies of the same primitive.

`components.json` points the CLI at this directory. Both Vite configurations and
TypeScript resolve `@/components/ui/*` here and `@/lib/utils` to the shared `cn`
helper. Existing application-specific aliases still work.

Tailwind is installed through `@tailwindcss/vite` in both builds. Styles are in
`shared/css/ui-tailwind.css`: theme and utility layers, explicit shared UI source
scanning, and semantic colors mapped to Mint's tokens. Preflight is omitted to
preserve the application's base styles. New shared primitives belong in this
folder so their utility classes are included automatically. To add one:

```sh
npx shadcn@latest add <component-name>
npm run typecheck
npm run build:ui
```

No project reinitialization is needed. Setup references:
[Tailwind with Vite](https://tailwindcss.com/docs/installation/using-vite) and
[shadcn manual installation](https://ui.shadcn.com/docs/installation/manual).

## VoicePoweredOrb

The supplied shader and audio-to-motion mapping are retained in
`voice-powered-orb.tsx`; sizing and graphics fallback are in its CSS.
`GeminiLiveOverlay` integrates it with Mint's live status, transcripts, voice
selection, microphone pause/resume and end-call callbacks.

The call supplies `analyserRef` from `useGeminiLiveVoice`. It mixes the existing
microphone and playback nodes for visualization without acquiring another mic.
Without this prop, the standalone `demo.tsx` requests its own mic only after Start.
That mode owns and releases its stream and AudioContext; the live component owns
neither. No external images or assets are required.

ResizeObserver handles the container, DPR is capped at two and applied once, and
reduced motion freezes shader time, rotation and distortion. If WebGL is unavailable,
the CSS ring and all textual call states/controls remain available. Ending a call
releases capture, playback, the analyser, audio context, WebGL resources and RAF.

Browser regressions can be served with `npm run dev:desktop:ui` and opened at
`/tests/liveOrb.browser.html` and `/tests/liveVoice.browser.html`. The first checks
the real shader and call UI; the second injects the external audio/session boundary
to verify cancellation of pending permission/connection and late events. They do
not make a real Gemini call. `/tests/liveOrb.browser.html?preview=1` shows a fixture
conversation with an injected audio level for layout review.
