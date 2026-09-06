# Release Notes - Mint Agent v1.14.0

## UI Stylesheet Cleanup & Theme Token Consistency

- **Removed the legacy `mint-*` layout CSS** (~330 lines per copy) from `src/renderer/src/index.css` and `src/renderer/src-web/index.css`:
  - `.mint-app`, `.mint-sidebar`, `.mint-toolbar`, `.mint-composer`, `.mint-message`, `.mint-model`, `.mint-picture(s)`, `.mint-empty`, `.mint-workspace` and their `@media` block were dead — no component renders those classes anymore.
  - Fixed a real styling conflict: the leftover `.mint-attachment` / `.mint-attachment button` rules were overriding the current composer-attachment styles in `chat.css` (transparent remove button, stray slate background/padding). The attachment strip now renders exactly as `chat.css` defines it, including the red circular remove button.
  - Removed the duplicate `.chat-inline-code` definition (two conflicting blocks in the same file) and the unused `.chat-ordered-list` / `.chat-unordered-list` rules.

- **Hard-coded colors replaced with theme tokens** so chat tables, code blocks, and error banners now follow the active theme (dark/light/midnight) instead of assuming a dark background:
  - `.mint-error` now derives from `--status-error` / `--text-main` via `color-mix`.
  - `.chat-table*` and `.chat-code-*` rules use `--border`, `--border-light`, `--shadow-sm`, and `color-mix` tints of `--text-main` instead of `rgba(255,255,255,…)` values that were invisible on the light theme.
  - Markdown table renderer (`src/renderer/shared/utils/markdown.tsx`) inline styles (container border/shadow, header tint, zebra rows, row borders) — these override the CSS, so they are now token-based too, fixing themed tables on Desktop and Web at once.
  - `body`/`html` base text color now uses `var(--text-main)` instead of a fixed dark-theme hex.

## Web Search Image Hotlink Protection & Direct URL Resolution

- **Unwrapped Web Search Image Proxy URLs**:
  - Automatically unwraps Brave Search internal proxy URLs (`imgs.search.brave.com`) back to direct source image URLs in both backend and frontend, eliminating 403 Forbidden errors.
  - Fixes images failing to display in Web and Desktop message bubbles while appearing as raw Markdown links in the CLI.
  - Automatically decodes existing image URLs stored in past conversation history upon rendering.

- **Referrer Policy Hardening**:
  - Added `referrerPolicy="no-referrer"` to image tags across Markdown bubbles, Image Search tiles, and Search Source drawer thumbnails to bypass third-party image hotlink protection.
