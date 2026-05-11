# Mint Release Notes

## v1.4.1 - CLI Agent Hardening & Stability

Mint 1.4.1 focuses on making the CLI and coding agent behave more like a real agent, while tightening several unsafe execution paths and improving reliability across provider selection, workspace handling, and action execution.

### Highlights

- CLI one-shot chat now executes returned actions, not just text responses
- Code Mode now uses only supported coding providers instead of silently falling through
- Agent loop is more resilient when a model returns malformed JSON
- Workspace detection is safer and no longer matches sibling paths by prefix
- Multiple shell execution paths were hardened to reduce command injection risk

### CLI & Agent Improvements

- **One-shot CLI action execution**
  - `mint chat <message>` and equivalent one-shot paths now execute structured actions the same way as interactive chat.
  - This fixes cases where Mint would say it was opening or searching for something but would not actually perform the action.

- **Better Code Mode provider routing**
  - Code Mode now selects from providers that are actually supported by the coding workflow:
    - `gemini`
    - `anthropic`
    - `openai`
    - `local_openai`
  - Unsupported coding providers such as `ollama` or `huggingface` no longer appear to be active while silently falling back elsewhere.

- **More robust agent loop**
  - Code Mode now attempts JSON repair when a provider returns malformed structured output.
  - Instead of failing immediately on the first invalid JSON response, Mint asks the model to reformat the reply into valid JSON and retries.

- **Accurate step reporting**
  - Code Mode now reports the actual number of executed steps instead of always showing the maximum configured step count.

### Security & Safety Fixes

- **Hardened URL and file opening paths**
  - Replaced shell-string execution with argument-based process execution in browser and file action handlers.

- **Hardened app launching**
  - `open_app` no longer builds launcher commands through shell interpolation for common launch paths.

- **Hardened Docker plugin**
  - Docker actions now use argument-based execution instead of shell-string concatenation.

- **Safer workflow process matching**
  - Custom workflow process-name matching now escapes regex characters before building a matcher.

### Workspace & Config Fixes

- **Workspace path matching fixed**
  - Workspace detection no longer treats sibling folders with the same prefix as the same project.
  - Example: `/project` no longer incorrectly matches `/project-two`

- **Workspace test isolation**
  - Workspace storage can now be redirected for tests through an override path, making tests more reliable in restricted environments.

- **Proactive cooldown config consistency**
  - Proactive suggestion cooldown now reads from the same config source as the rest of the app.

- **Provider fallback logic improved**
  - Non-Gemini providers are no longer blocked by a missing Gemini API key before routing even begins.

### Documentation

- Rewrote the README to better reflect Mint’s current state as a desktop assistant plus CLI coding agent
- Clarified current agent capabilities, supported workflows, and provider behavior

### Testing

- Expanded regression coverage for:
  - Docker plugin execution
  - Provider routing helpers
  - Code agent helper behavior
  - Workspace path boundary handling

- Current test status:
  - **77 tests passed**

### Notes

Mint 1.4.1 is not a major feature release. It is a stabilization release aimed at making existing agentic workflows safer, more honest about what provider is actually being used, and more reliable in day-to-day CLI usage.
