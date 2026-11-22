import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, X } from "lucide-react";
import type { AgentCli, CreateTabRequest } from "../types";

interface NewTabModalProps {
  onClose: () => void;
  onCreate: (req: CreateTabRequest) => Promise<void>;
}

export function NewTabModal({ onClose, onCreate }: NewTabModalProps) {
  const [cli, setCli] = useState<AgentCli>("claude");
  const [repoPath, setRepoPath] = useState("");
  const [baseBranch, setBaseBranch] = useState("main");
  const [slug, setSlug] = useState("feat-");
  const [initialPrompt, setInitialPrompt] = useState("");
  const [skipWorktree, setSkipWorktree] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function pickRepo() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setRepoPath(selected);
      try {
        const branch = await invoke<string>("detect_git_branch", {
          repoPath: selected,
        });
        setBaseBranch(branch);
      } catch {
        // fallback to main
      }
    }
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!repoPath.trim()) {
      setError("Select or enter a target Git repository path.");
      return;
    }
    if (!slug.trim() || slug.trim() === "feat-") {
      setError("Enter a feature slug (e.g. feat-auth).");
      return;
    }
    setBusy(true);
    try {
      await onCreate({
        cli,
        repo_path: repoPath.trim(),
        base_branch: baseBranch.trim() || "main",
        slug: slug.trim(),
        initial_prompt: initialPrompt.trim(),
        skip_worktree: skipWorktree,
      });
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Spawn New Agent Worktree Tab</h3>
          <button className="btn" style={{ padding: "4px 6px" }} onClick={onClose}>
            <X size={15} />
          </button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="modal-body">
            {error && <div className="error-box">{error}</div>}

            <div className="field">
              <label>Agent CLI</label>
              <select
                value={cli}
                onChange={(e) => setCli(e.target.value as AgentCli)}
              >
                <option value="claude">Claude Code (claude --mcp-config)</option>
                <option value="codex">OpenAI Codex CLI (codex)</option>
                <option value="antigravity">Antigravity CLI (agy)</option>
                <option value="shell">Plain OS Shell (manual launch)</option>
              </select>
            </div>

            <div className="field">
              <label>Target Git Repository</label>
              <div className="field-row">
                <input
                  style={{ flex: 1 }}
                  placeholder="C:\Users\you\projects\my-app"
                  value={repoPath}
                  onChange={(e) => setRepoPath(e.target.value)}
                />
                <button type="button" className="btn" onClick={pickRepo}>
                  <FolderOpen size={14} /> Browse
                </button>
              </div>
            </div>

            <div className="field-row">
              <div className="field" style={{ flex: 1 }}>
                <label>Feature Slug</label>
                <input
                  placeholder="feat-auth"
                  value={slug}
                  onChange={(e) => setSlug(e.target.value)}
                />
              </div>
              <div className="field" style={{ flex: 1 }}>
                <label>Active Dev Branch</label>
                <input
                  placeholder="main"
                  value={baseBranch}
                  onChange={(e) => setBaseBranch(e.target.value)}
                />
              </div>
            </div>

            <div className="field">
              <label>Initial Prompt (auto-written to PTY after launch)</label>
              <textarea
                rows={3}
                placeholder="Implement OAuth PKCE login, capture Playwright screenshots, and call submit_changelog when verified."
                value={initialPrompt}
                onChange={(e) => setInitialPrompt(e.target.value)}
              />
            </div>

            <label
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                fontSize: 12,
                color: "var(--muted)",
              }}
            >
              <input
                type="checkbox"
                checked={skipWorktree}
                onChange={(e) => setSkipWorktree(e.target.checked)}
              />
              Open directly in folder without creating a new git worktree
            </label>
          </div>

          <div className="modal-footer">
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy}>
              {busy ? "Creating Worktree..." : "Launch Agent Tab"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
