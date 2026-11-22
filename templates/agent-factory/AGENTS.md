# Grex Central Agent Instructions (`~/.agent-factory/AGENTS.md`)

You are operating inside a **Grex Local Software Factory** worktree. Multiple autonomous agents may be working concurrently on sibling worktrees from the same repository.

## Mandatory Operating Rules

1. **Stay Inside Your Worktree**
   - Your working directory is `.worktrees/<slug>` on branch `agent/<slug>`.
   - Never checkout or mutate the base branch directly.
   - Before editing shared files, call the `get_factory_state` MCP tool to inspect active worktrees, current dev branch `HEAD`, and files touched by sibling agents.

2. **Check Environment & MCP Connection**
   - `AGENT_FEATURE_ID`: Your assigned feature identifier (pass this to `submit_changelog` and `ping_user`).
   - `FACTORY_MCP_PORT` / `FACTORY_MCP_URL`: Local Grex MCP server endpoint.

3. **Update the Issue Board Like a Human**
   - Use `list_issues` to inspect open specs and requirements.
   - Use `update_issue(issue_id, status?, comment?, branch_name?, author?)` to move your assigned issue (`Open` -> `InProgress` -> `InReview` -> `Done`), post concise progress updates, and record your chosen feature branch name.
   - **Explicit User Permission Required to Add/Edit Issues:** `create_issue` and `update_issue` spec/label/title edits (`append_spec`, `add_labels`, `title`, `priority`) are available so you can add details or file issues when asked — **ONLY** use those to add content to issues or create new issues when **explicitly told to do so by the user**. Never file unsolicited issues or mutate specs on your own.

4. **Unblock Early via `ping_user`**
   - If you hit an ambiguous requirement, missing credential, or conflicting edit with another agent, call `ping_user(message, urgency)` immediately so the operator's tab highlights with an OS alert.

4. **Deliver via the 5-Subtab HTML Changelog Contract**
   - Before finishing, generate a self-contained HTML changelog (inline CSS/JS + embedded base64 screenshots or relative paths inside your worktree) containing all 5 required subtabs:
     1. `summary` — Summary
     2. `architecture` — Architecture
     3. `ui-before-after` — UI Before/After
     4. `user-stories` — User Stories (with verification screenshots)
     5. `test-verification` — Test Verification
   - Call `submit_changelog(feature_id, html_path)` to validate the HTML and enqueue it into the Grex Active Review Queue.
   - If the reviewer requests changes, feedback will be piped directly into your terminal. Apply fixes, re-verify, regenerate the changelog, and re-call `submit_changelog`.

5. **Available Skills**
   - `.skills/worktree-workflow/SKILL.md`: Branch isolation, screenshot capture, exact HTML template, and MCP submission contract.
   - `.skills/factory-self-improve/SKILL.md`: How to evolve `~/.agent-factory/AGENTS.md` and shared skills atomically via `update_central_instructions`.
