# Mint Release Notes

## v1.5.3

This release focuses on the desktop assistant experience: a cleaner navigation shell, a local-only Pictures library for sent images, safer chat history behavior, and more complete appearance controls.

### Desktop Navigation, Local Pictures, and Privacy

Mint Desktop now has a more workspace-like layout with a collapsible sidebar, clearer Chat/Pictures navigation, startup polish, and safer reset actions. Image attachments sent from the desktop chat can now be kept locally as files without storing raw image data in chat history.

#### Highlights

- **Collapsible Sidebar by Default:** The desktop app now starts with the sidebar collapsed, keeping the chat/model workspace focused on launch.
- **Chat and Pictures Navigation:** Added explicit `Chat` and `Pictures` sidebar actions for switching between the conversation and local image gallery.
- **New Chat Confirmation:** Renamed `New Conversation` to `New Chat` and added confirmation before clearing the current conversation.
- **Clear Confirmation:** The `Clear` action now asks for confirmation before deleting chat history.
- **Smoother Page Switching:** Added fade/slide transitions when switching between Chat and Pictures.
- **Improved Pictures Gallery:** Added a cleaner local gallery layout with larger cards, hover states, and a more polished header.
- **Local Pictures Library:** Desktop images are saved locally under `~/.config/mint/Pictures` after the user sends a message with an image.
- **Pictures Metadata Index:** Saved images are indexed in `~/.config/mint/Pictures/pictures.json` so the desktop gallery can list them.
- **No Raw Image History:** Chat history replaces image payloads with a text placeholder instead of storing raw image base64 data.
- **Timestamp Preservation:** Chat history now preserves original message timestamps across app restarts and Gemini history syncs.
- **Theme-aware Loading Screen:** Startup loading now follows the active theme, accent color, and text color.
- **Theme & UI Settings:** Added UI font size control alongside theme, accent color, system text color, glass blur, and font family.
- **README Polish:** README sections were reorganized, made collapsible where useful, and updated with new desktop/privacy feature notes.

#### Local Pictures Library

Desktop image attachments now use a local-first storage flow:

- Images are saved only after the user sends a message.
- Files are written to `~/.config/mint/Pictures`.
- Metadata is written to `~/.config/mint/Pictures/pictures.json`.
- The in-app `Pictures` page reads that local index and renders saved images as a gallery.
- Proactive screen captures are not saved to the Pictures library.

Example saved files:

```text
~/.config/mint/Pictures/
├── mint-20260525-034738-7c0a9739.png
└── pictures.json
```

#### Chat History Privacy

Desktop image payloads are no longer stored as raw base64 inside `~/.config/mint/mint-chat-history.json`.

When a history entry contains image data, Mint stores a placeholder such as:

```text
[Image omitted from chat history; saved locally when sent by the user.]
```

The actual image file remains in the local Pictures folder when it was sent by the user.

#### Desktop Appearance Updates

- Sidebar starts collapsed on launch.
- Sidebar actions now include `New Chat`, `Chat`, `Pictures`, model controls, assistant shortcuts, `Clear`, and `Settings`.
- `New Chat` and `Clear` are guarded by confirmation prompts.
- Pictures view uses a dedicated gallery page instead of being mixed into the chat surface.
- Light, dark, midnight, and custom themes now apply more consistently across loading, chat, sidebar, and gallery surfaces.
- Settings now include `Font Size` options: Small, Medium, Large, and Extra Large.


#### Interactive CLI UX

- Clipboard images are now attached first and sent only when the user presses Enter.
- Pending image labels are rendered inside the input box.
- Multiple pending images are shown as compact labels:
  - `[Image #1] [Image #2]`
- Fixed terminal input edge cases where `Ctrl+V` could insert stray `v` characters.
- Long pasted text no longer breaks the input border or terminal layout.
- Pasted content can be combined with typed text:
  - text before paste
  - pasted content
  - text after paste
- Image attachments follow the same composition model:
  - text before image
  - image labels
  - text after image
- Code Agent `search_code` can now scope searches to a relative file or folder with `input.path`, avoiding whole-project scans when the likely area is known.
- Scoped search activity now displays the target folder, for example `Search requestApproval in src/CLI`.
- Code Agent search heuristics now inspect or use the current project layout before choosing a scoped search path, so Mint works better across projects with different structures.

#### Provider and Vision Handling

- Gemini image MIME handling now preserves the real image MIME type instead of assuming PNG.
- Chat provider paths can receive multiple images in one request where supported.
- Code Agent multimodal requests now pass attached image context to supported providers.

### Current Test Status

- **190 tests passed**
- **29 test suites passed**
