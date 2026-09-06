---
name: github-account-recon
description: Investigate a GitHub user/organization's repos — verify a specific repo exists, detect forks (and their upstreams) via the API, and spot renamed repos via HTTP 301 redirects on the old URL. Trigger when the user asks to find a GitHub repo/account, check whether someone forked a project, list a user's public repos, or 'ทำไมหา repo ของ X ไม่เจอ' (Thai).
revisions: 1
---

# GitHub Account Recon

Answer "does this repo exist / did X fork Y / why can't I find it under the expected name?" with real evidence from the GitHub API. The most common failure is a **rename**: the repo exists but under a different name than expected, so name filters and searches miss it.

## Core move: detect a rename with a 301

GitHub returns **HTTP 301** when you hit a repo URL whose name was changed, redirecting to the new location. This is the single most useful trick in this skill:

- `curl -sI https://github.com/<owner>/<expected-name>` → look at the status line and `location:` header.
  - **301 + location to a new repo path** → repo was **renamed** (old links still work; it is not deleted).
  - **404** → repo really does not exist under that owner/name.
  - **200** → exists exactly as named.
- Why this matters: a filtered list of forks (e.g. matching `aipass-bridge`) will silently miss a fork renamed to `ai-passport-bridge`. The 301 is how you prove the rename instead of guessing.
- `curl -sI` with the bash tool, or `Invoke-WebRequest -Method Head` on Windows. GitHub allows unauthenticated HEAD requests.

## Workflow

1. **Pin the owner and the expected repo name.** If the user says "repo ของ X" and only gives a partial/remembered name, treat the name as provisional.

2. **List public repos via the API** (works when web search tools are down):
   ```
   curl -s https://api.github.com/users/<owner>/repos?per_page=100&sort=updated
   ```
   Extract name, fork status, description, default_branch, pushed_at/created_at, stars. Fields to pull: `name`, `full_name`, `fork`, `parent` (present only on forks), `description`, `created_at`, `pushed_at`, `stargazers_count`.
   - For orgs: `https://api.github.com/orgs/<org>/repos`.
   - Rate limit: unauthenticated is 60 req/hr per IP — fine for a handful of calls; if you hit it, tell the user and slow down rather than scraping.

3. **Check the exact match on the API list.** Compare the expected name against actual `name` values. A near-miss (`ai-passport-bridge` vs expected `aipass-bridge`) is the classic miss — the 301 trick confirms it.

4. **Confirm fork parent.** For a suspected fork, read the `parent.full_name` field from the repo's API response: `curl -s https://api.github.com/repos/<owner>/<repo>` → `fork: true` and `parent: {full_name: "<upstream>"}`. Also note `source` (the original root if re-forked).

5. **Report with sources and structure.**
   - A table: repo → status (original / fork of X) → last push → notes.
   - Label every fact with its source: "from GitHub API", "from the 301 redirect", "from web search". **Never invent repos or dates.**
   - If a web-search attempt failed mid-task, say so and show that you fell back to direct API calls — the user should know which facts are API-verified.

6. **Answer in the user's language.** Thai trigger ("เจอ repo ของ X มั้ย / ทำไมหาไม่เจอ") → reply in Thai. Tone: friendly, celebratory when found, with the concrete explanation of why earlier attempts missed it.

## Gotchas

- **Search/filters match substrings, not intent.** Filtering a fork list for `aipass-bridge` misses `ai-passport-bridge` even though it is the repo the user means. Always reconcile expected vs actual names before concluding "not found".
- **301 means rename, not deletion.** People assume a missing repo was deleted; the redirect proves it was renamed and old links are preserved. State this explicitly.
- **`parent` is absent unless the repo is a fork.** Don't claim "original" without checking `fork: false` on the API response.
- **Web search / Brave can fail** (`web search request failed`). The GitHub REST API and `curl -I` need no search backend — go straight to them.
- **Don't trust one date field alone.** `created_at` = when the repo was made (or first pushed); `pushed_at` = last activity. A rename resets neither.

## Output shape

1. Verdict line: found / not found / exists-but-renamed, with the exact `full_name`.
2. Evidence table (name, fork/original with parent, dates, stars).
3. The rename proof when applicable: old URL → HTTP status → redirect target.
4. Broader account context only if asked (count of repos, recent ones) — don't dump the whole account unprompted.
5. Offer next steps (diff fork vs upstream, inspect latest commit) in the user's language.
