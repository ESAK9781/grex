---
name: worktree-workflow
description: Enforces Git worktree isolation, UI screenshot capture, the 5-subtab HTML changelog contract, and Grex MCP submission.
---

# Worktree Workflow & 5-Subtab Changelog Contract

## 1. Branch & Conflict Isolation
1. Confirm you are inside `.worktrees/<slug>` on branch `agent/<slug>`.
2. Call `get_factory_state()` before starting major edits to see what files other active agents have modified.
3. Commit your work incrementally on `agent/<slug>` with clear commit messages. Never force-push or touch the base branch.

## 2. Capturing UI Verification Screenshots
When your feature affects UI:
1. Capture a **Before** screenshot (from base branch or prior state) and an **After** screenshot using Playwright or headless Chromium:
   ```bash
   npx playwright screenshot http://localhost:3000 .grex/after.png --full-page
   ```
2. Embed screenshots directly as `data:image/png;base64,...` (preferred for portability) or reference them via relative paths in your worktree (Grex serves changelogs via Tauri's `asset://` protocol).

## 3. Exact 5-Subtab HTML Template
Write your changelog to `.grex/changelog.html` (or `<feature_id>-changelog.html`).
Grex validates your HTML using `scraper` to verify all **5 required subtabs** exist (`data-tab` attribute values):
- `summary`
- `architecture`
- `ui-before-after`
- `user-stories`
- `test-verification`

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Changelog — FEATURE_ID</title>
  <style>
    body { font-family: system-ui, sans-serif; background: #0d1117; color: #e6edf3; margin: 0; }
    nav { display: flex; gap: 8px; padding: 12px 16px; border-bottom: 1px solid #30363d; background: #161b22; }
    nav button { background: transparent; color: #8b949e; border: 1px solid transparent; padding: 6px 12px; border-radius: 6px; cursor: pointer; }
    nav button.active { color: #e6edf3; background: #21262d; border-color: #30363d; }
    section[data-tab] { display: none; padding: 20px; max-width: 960px; }
    section[data-tab].active { display: block; }
  </style>
</head>
<body>
  <nav>
    <button data-tab-btn="summary" class="active">Summary</button>
    <button data-tab-btn="architecture">Architecture</button>
    <button data-tab-btn="ui-before-after">UI Before/After</button>
    <button data-tab-btn="user-stories">User Stories</button>
    <button data-tab-btn="test-verification">Test Verification</button>
  </nav>
  <section data-tab="summary" class="active"><h2>Summary</h2><p>...</p></section>
  <section data-tab="architecture"><h2>Architecture</h2><p>...</p></section>
  <section data-tab="ui-before-after"><h2>UI Before/After</h2><p>...</p></section>
  <section data-tab="user-stories"><h2>User Stories</h2><p>...</p></section>
  <section data-tab="test-verification"><h2>Test Verification</h2><pre>...</pre></section>
  <script>
    document.querySelectorAll('[data-tab-btn]').forEach(btn => {
      btn.addEventListener('click', () => {
        const t = btn.dataset.tabBtn;
        document.querySelectorAll('[data-tab-btn]').forEach(b => b.classList.toggle('active', b.dataset.tabBtn === t));
        document.querySelectorAll('section[data-tab]').forEach(s => s.classList.toggle('active', s.dataset.tab === t));
      });
    });
  </script>
</body>
</html>
```

## 4. Submitting to the Active Review Queue
Call the MCP tool:
```json
{
  "tool": "submit_changelog",
  "arguments": {
    "feature_id": "<AGENT_FEATURE_ID>",
    "html_path": "<absolute-path-to-changelog.html>"
  }
}
```
If validation fails, the tool returns the exact missing subtabs. Fix and re-submit.
