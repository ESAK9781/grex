---
name: factory-self-improve
description: Documents the Grex central factory directory (~/.agent-factory/), MCP tool schemas, and safe atomic updates via update_central_instructions.
---

# Factory Self-Improvement Skill

Use this skill when the operator asks you to improve the factory's own instructions, add new shared skills, or refine review templates.

## 1. Central Directory Layout (`~/.agent-factory/`)
`~/.agent-factory/` is a Git-tracked repository symlinked into every agent worktree:
```text
~/.agent-factory/
├── AGENTS.md                           # Global rules loaded by all agents
└── .skills/
    ├── worktree-workflow/
    │   └── SKILL.md                    # Branch isolation + 5-subtab changelog spec
    └── factory-self-improve/
        └── SKILL.md                    # This file
```

## 2. Embedded Grex MCP Server Tools

| Tool | Arguments | Behavior |
|---|---|---|
| `submit_changelog` | `{ feature_id: string, html_path: string }` | Validates all 5 subtabs with `scraper`, enqueues into SQLite Active Review Queue, fires OS notification. |
| `ping_user` | `{ message: string, urgency: "low"\|"normal"\|"high"\|"critical", feature_id?: string }` | Highlights the agent tab in Grex UI and sends an OS alert. |
| `get_factory_state` | `{}` | Returns active worktrees, dev branch HEAD, agent states, and files touched by every active agent. |
| `update_central_instructions` | `{ target_file: string, content: string, commit_message?: string }` | Atomically writes `target_file` inside `~/.agent-factory/` (blocking path traversal) and commits to git history. |

## 3. Safely Evolving Central Config
Always use `update_central_instructions` rather than editing symlinked files directly:
```json
{
  "tool": "update_central_instructions",
  "arguments": {
    "target_file": ".skills/rust-testing/SKILL.md",
    "content": "---\nname: rust-testing\n---\n...",
    "commit_message": "Add rust-testing shared skill"
  }
}
```
Every update is written atomically via temp-file rename and recorded in `~/.agent-factory`'s Git log so any change can be audited or reverted.
