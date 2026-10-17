# Release Notes - Mint Agent v1.17.0

Released: 2026-10-17

## Terminal and Web launch

- `/autoskill` now marks the active setting as `on (current)` or `off (current)` in its selection menu.
- `/mint-web` now opens the default browser after both the Web UI and API are ready. It displays `Mint Web: http://localhost:9000` with the URL in blue in the CLI, followed by `Your browser will open automatically when ready.`
- Added `mint web --open` to open the browser automatically from the shell.
- Repeating `/mint-web` reuses the running server and clearly reports that it is already running instead of promising another automatic browser launch.

## Voice input

- Web dictation now retains all finalized phrases across speech-recognition updates without duplicating revised results.
- Desktop microphone input now appends the transcript to the current composer draft instead of automatically sending a chat message. Users can review and edit the text before pressing Send.
- Canceled recordings and empty transcripts leave the composer draft unchanged.
- Switching conversations cancels pending dictation and prevents a delayed transcript from entering another chat's draft.
- Browsers without speech recognition, such as Firefox, now show a popup recommending Chrome or Edge when the microphone button is pressed.
