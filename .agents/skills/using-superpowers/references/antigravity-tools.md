# Antigravity CLI (`agy`) Tool Mapping

Skills speak in actions ("dispatch a subagent", "create a todo", "read a file"). On the Antigravity CLI (`agy`) these resolve to the tools below.

| Action skills request | Antigravity CLI equivalent |
|----------------------|----------------------|
| Read a file | `view_file` |
| Create a new file | `write_to_file` |
| Edit a file | `replace_file_content` |
| Edit a file in several places at once | `multi_replace_file_content` |
| Run a shell command | `run_command` |
| Search file contents | `grep_search` |
| Find files by name / list a directory | `list_dir` (no dedicated glob tool — combine `list_dir` with `grep_search`) |
| Fetch a URL | `read_url_content` |
| Search the web | `search_web` |
| Pose a structured question to your human partner | `ask_question` |
| Dispatch a subagent (`Subagent (general-purpose):` template) | `invoke_subagent` with a built-in `TypeName` — `self` for full-capability work, `research` for read-only (see [Subagent support](#subagent-support)) |
| Multiple parallel dispatches | Multiple entries in one `invoke_subagent` call's `Subagents` array |
| Task tracking ("create a todo", "mark complete") | a **task artifact** — `write_to_file` with `IsArtifact: true` and `ArtifactType: "task"` (see [Task tracking](#task-tracking)). **Not** `manage_task`, which manages background processes. |

## Invoking a skill — read its `SKILL.md`

Antigravity surfaces every installed skill's `name` + `description` to you at the start of each session, but it has **no `Skill`/`activate_skill` tool**. To load a skill, **read its `SKILL.md` with `view_file`, setting `IsSkillFile: true`** when the skill applies — e.g. `view_file` on `.../plugins/superpowers/skills/<skill-name>/SKILL.md` with `IsSkillFile: true`. (`IsSkillFile` is agy's own signal that you're reading a file to *execute its instructions*, not to edit or preview it — set it whenever you load a skill.)

This is the blessed skill-loading mechanism on this harness. The general rule "never read skill files manually" means "don't bypass your platform's skill-loading mechanism" — and on Antigravity, reading `SKILL.md` *is* that mechanism. Reading it honors the rule rather than breaking it.

You already know which skills exist and what they're for: their names and descriptions are in front of you at session start. When a description matches what you're about to do, read that skill's `SKILL.md` before acting.
