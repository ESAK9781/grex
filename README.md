# Grex — Local Rust Agent Software Factory

**Grex** is a high-performance desktop software factory built with **Rust**, **Tauri v2**, **portable-pty**, **SQLite**, and an **embedded Axum MCP server**. Run dozens of coding agents (`claude`, `codex`, `antigravity`, or any shell CLI) concurrently in isolated Git worktrees with real native pseudo-terminals, structured 5-subtab HTML changelog reviews, and atomic central `AGENTS.md` + meta-skills management.

- **Landing & Install Site:** https://esak9781.github.io/grex/
- **Releases:** https://github.com/ESAK9781/grex/releases

---

## Architecture

### 1. Core Rust Architecture
- **Desktop Shell / UI**: Tauri v2 (Rust backend + React/TypeScript frontend) rendering **xterm.js** natively alongside a sandboxed `<iframe>` (via Tauri's `asset://` protocol) for HTML changelogs.
- **PTY & Process Management**: `portable-pty` (ConPTY on Windows, `openpty` on macOS/Linux) + `tokio` streaming bytes asynchronously over Tauri events to `xterm.js`.
- **State & Queue Persistence**: Bundled SQLite (`rusqlite`) + `serde_json` persisting active agent tabs, worktree paths, agent states (`Working`, `NeedsReview`, `Blocked`, `Approved`, `Archived`), and review iterations.

### 2. Embedded Rust MCP Server (`axum` + `tokio` + `grex-mcp` stdio shim)
Grex embeds a local MCP server on `127.0.0.1:4739` (with Streamable HTTP `POST /mcp`, SSE `GET /sse`, and a lightweight `grex-mcp` stdio shim binary for `--mcp-config`):
- `submit_changelog(feature_id, html_path)` — Validates all 5 required subtabs with `scraper`, enqueues into the Active Review Queue, and fires an OS notification (`tauri-plugin-notification`).
- `ping_user(message, urgency, feature_id?)` — Highlights the agent's tab in the UI and fires an OS alert when blocked.
- `get_factory_state()` — Serializes active worktrees, current dev branch `HEAD`, and files touched by sibling agents to prevent conflicts.
- `update_central_instructions(target_file, content, commit_message?)` — Atomically updates `~/.agent-factory/AGENTS.md` or `.skills/` with path-traversal protection and Git commit history.

### 3. Git Worktree & Tab Lifecycle
1. Select CLI (`claude`, `codex`, `antigravity`, `shell`), target repo, active dev branch, and feature slug (`feat-auth`).
2. Creates `.worktrees/<slug>` on `agent/<slug>` via `git worktree add`.
3. Symlinks central `AGENTS.md` and `.skills/` (with graceful fallback), writes `.grex-mcp.json`, and injects `AGENT_FEATURE_ID` and `FACTORY_MCP_PORT` into the PTY environment.
4. Auto-launches the selected CLI + initial prompt directly into the PTY master writer.

### 4. 5-Subtab HTML Changelog Contract & Review Loop
Every changelog is validated for 5 required sections:
1. `summary` — Summary
2. `architecture` — Architecture
3. `ui-before-after` — UI Before/After
4. `user-stories` — User Stories (with verification screenshots)
5. `test-verification` — Test Verification

- **Request Changes** writes reviewer feedback directly into the agent's PTY master write handle (`pty_writer.write_all(...)`).
- **Approve & Merge** runs `git merge --no-ff agent/<slug>`, removes the worktree (`git worktree remove --force`), deletes the merged branch, and archives the session.

### 5. Central `~/.agent-factory/` & Meta-Skills
Bootstrapped automatically on first launch as a Git-tracked directory:
- `AGENTS.md`
- `.skills/worktree-workflow/SKILL.md`
- `.skills/factory-self-improve/SKILL.md`

---

## Quick Install

### Windows (PowerShell)
```powershell
irm https://esak9781.github.io/grex/install.ps1 | iex
```

### macOS / Linux
```bash
curl -fsSL https://esak9781.github.io/grex/install.sh | sh
```

### Build from Source
```bash
git clone https://github.com/ESAK9781/grex.git
cd grex
npm install
npm run tauri build
```

## Development
```bash
npm install
npm run tauri dev
```

Run Rust unit tests:
```bash
cargo test --manifest-path src-tauri/Cargo.toml
```
