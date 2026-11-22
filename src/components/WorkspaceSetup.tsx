import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FolderGit2,
  FolderOpen,
  GitBranch,
  Github,
  History,
  Terminal,
} from "lucide-react";
import type { AgentCli, AppSettings, WorkspaceInfo } from "../types";

interface WorkspaceSetupProps {
  settings: AppSettings;
  onWorkspaceReady: (ws: WorkspaceInfo) => void;
  onSaveSettings: (next: AppSettings) => Promise<void>;
}

export function WorkspaceSetup({
  settings,
  onWorkspaceReady,
  onSaveSettings,
}: WorkspaceSetupProps) {
  const [dirPath, setDirPath] = useState("");
  const [pendingInfo, setPendingInfo] = useState<WorkspaceInfo | null>(null);
  const [cloneUrl, setCloneUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function inspectAndSelect(path: string) {
    setError(null);
    setBusy(true);
    try {
      const info = await invoke<WorkspaceInfo>("set_session_workspace", {
        path,
      });
      if (info.is_git_repo) {
        onWorkspaceReady(info);
      } else {
        setPendingInfo(info);
        setDirPath(info.path);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleBrowse() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setDirPath(selected);
      await inspectAndSelect(selected);
    }
  }

  async function handleInitGit() {
    if (!dirPath.trim()) return;
    setError(null);
    setBusy(true);
    try {
      const info = await invoke<WorkspaceInfo>("init_workspace_git", {
        path: dirPath.trim(),
      });
      onWorkspaceReady(info);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleClone() {
    if (!dirPath.trim() || !cloneUrl.trim()) return;
    setError(null);
    setBusy(true);
    try {
      const info = await invoke<WorkspaceInfo>("clone_workspace_repo", {
        path: dirPath.trim(),
        repoUrl: cloneUrl.trim(),
      });
      onWorkspaceReady(info);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="setup-screen">
      <div className="setup-card">
        <div className="setup-header">
          <div className="brand-mark" style={{ width: 34, height: 34, fontSize: 15 }}>
            GX
          </div>
          <div>
            <h2>Select Session Workspace</h2>
            <p>
              Choose a project directory once for this session — or launch directly
              from your terminal via <code>grex ./</code>
            </p>
          </div>
        </div>

        {error && <div className="error-box">{error}</div>}

        <div className="field">
          <label>Project Directory</label>
          <div className="field-row">
            <input
              style={{ flex: 1 }}
              placeholder="C:\Users\you\projects\my-app"
              value={dirPath}
              onChange={(e) => setDirPath(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && dirPath.trim()) {
                  inspectAndSelect(dirPath.trim());
                }
              }}
            />
            <button className="btn" onClick={handleBrowse} disabled={busy}>
              <FolderOpen size={14} /> Browse
            </button>
            <button
              className="btn btn-primary"
              disabled={busy || !dirPath.trim()}
              onClick={() => inspectAndSelect(dirPath.trim())}
            >
              Open
            </button>
          </div>
        </div>

        {pendingInfo && !pendingInfo.is_git_repo && (
          <div className="git-setup-box">
            <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 4 }}>
              <FolderGit2 size={14} style={{ verticalAlign: -2, marginRight: 6 }} />
              <code>{pendingInfo.name}</code> is not a Git repository yet
            </div>
            <p style={{ margin: "0 0 12px", fontSize: 12, color: "var(--muted)" }}>
              Grex uses Git worktrees to isolate concurrent agents. Initialize a fresh
              repo here or clone from GitHub:
            </p>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <button
                className="btn btn-primary"
                style={{ justifyContent: "center" }}
                disabled={busy}
                onClick={handleInitGit}
              >
                <GitBranch size={14} /> Initialize New Git Repo (main)
              </button>
              <div className="field-row">
                <input
                  style={{ flex: 1 }}
                  placeholder="https://github.com/owner/repo or owner/repo"
                  value={cloneUrl}
                  onChange={(e) => setCloneUrl(e.target.value)}
                />
                <button
                  className="btn"
                  disabled={busy || !cloneUrl.trim()}
                  onClick={handleClone}
                >
                  <Github size={14} /> Clone Here
                </button>
              </div>
              <button
                className="btn btn-ghost"
                style={{ justifyContent: "center", fontSize: 11.5 }}
                onClick={() => onWorkspaceReady(pendingInfo)}
              >
                Continue without Git worktrees
              </button>
            </div>
          </div>
        )}

        <div className="field" style={{ marginTop: 4 }}>
          <label>
            <Terminal size={12} style={{ verticalAlign: -2, marginRight: 5 }} />
            Global Default Agent CLI (used for 1-click issue spawning)
          </label>
          <div className="cli-picker">
            {(["claude", "codex", "antigravity", "shell"] as AgentCli[]).map(
              (cli) => (
                <button
                  key={cli}
                  className={`btn ${settings.default_cli === cli ? "btn-primary" : ""}`}
                  onClick={() =>
                    onSaveSettings({ ...settings, default_cli: cli })
                  }
                >
                  {cli}
                </button>
              )
            )}
          </div>
        </div>

        {settings.recent_workspaces.length > 0 && (
          <div className="field">
            <label>
              <History size={12} style={{ verticalAlign: -2, marginRight: 5 }} />
              Recent Workspaces
            </label>
            <div className="recent-list">
              {settings.recent_workspaces.map((rp) => (
                <button
                  key={rp}
                  className="recent-item"
                  disabled={busy}
                  onClick={() => inspectAndSelect(rp)}
                >
                  <span>{rp}</span>
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
