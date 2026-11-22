import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  Code2,
  Download,
  ExternalLink,
  Plug,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";
import type { InstalledMcp } from "../types";

interface McpManagerModalProps {
  mcps: InstalledMcp[];
  onClose: () => void;
  onChanged: (msg: string) => void;
}

interface PresetMcp {
  id: string;
  name: string;
  repo: string;
  description: string;
  command: string;
  args: string[];
  envKeys?: string[];
}

const GITHUB_PRESETS: PresetMcp[] = [
  {
    id: "playwright",
    name: "Playwright MCP",
    repo: "microsoft/playwright-mcp",
    description: "Headless browser automation & UI Before/After screenshots",
    command: "npx",
    args: ["-y", "@playwright/mcp@latest"],
  },
  {
    id: "github",
    name: "GitHub MCP",
    repo: "modelcontextprotocol/servers",
    description: "Issues, PRs, branches, code search, and repo automation",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"],
    envKeys: ["GITHUB_PERSONAL_ACCESS_TOKEN"],
  },
  {
    id: "context7",
    name: "Context7 Docs",
    repo: "upstash/context7",
    description: "Up-to-date library documentation & code examples",
    command: "npx",
    args: ["-y", "@upstash/context7-mcp"],
  },
  {
    id: "filesystem",
    name: "Filesystem MCP",
    repo: "modelcontextprotocol/servers",
    description: "Scoped read/write/search tools for workspace directories",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "."],
  },
  {
    id: "sequential-thinking",
    name: "Sequential Thinking",
    repo: "modelcontextprotocol/servers",
    description: "Structured multi-step reasoning for complex architecture",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-sequential-thinking"],
  },
  {
    id: "memory",
    name: "Memory Graph",
    repo: "modelcontextprotocol/servers",
    description: "Persistent entity & knowledge-graph memory across sessions",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-memory"],
  },
  {
    id: "fetch",
    name: "Fetch MCP",
    repo: "modelcontextprotocol/servers",
    description: "Fetch & convert web pages to clean markdown (via uvx)",
    command: "uvx",
    args: ["mcp-server-fetch"],
  },
];

export function McpManagerModal({
  mcps,
  onClose,
  onChanged,
}: McpManagerModalProps) {
  const [tab, setTab] = useState<"installed" | "github" | "json">("github");
  const [repoInput, setRepoInput] = useState("");
  const [customId, setCustomId] = useState("");
  const [envText, setEnvText] = useState("");
  const [jsonInput, setJsonInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const installedIds = new Set(mcps.map((m) => m.id));

  function parseEnvLines(raw: string): Record<string, string> {
    const out: Record<string, string> = {};
    for (const line of raw.split(/\r?\n/)) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) continue;
      const idx = trimmed.indexOf("=");
      if (idx > 0) {
        out[trimmed.slice(0, idx).trim()] = trimmed.slice(idx + 1).trim();
      }
    }
    return out;
  }

  async function handleInstallGitHub(e: React.FormEvent) {
    e.preventDefault();
    if (!repoInput.trim()) return;
    setError(null);
    setBusy(true);
    try {
      const installed = await invoke<InstalledMcp>("install_mcp_from_github", {
        repoUrl: repoInput.trim(),
        customId: customId.trim() || null,
        env: parseEnvLines(envText),
      });
      setRepoInput("");
      setCustomId("");
      setEnvText("");
      setTab("installed");
      onChanged(
        `Installed ${installed.name} from GitHub & synced to active worktrees`
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleQuickPreset(preset: PresetMcp) {
    setError(null);
    setBusy(true);
    try {
      const env: Record<string, string> = {};
      for (const k of preset.envKeys ?? []) {
        env[k] = "";
      }
      await invoke("add_custom_mcp", {
        req: {
          id: preset.id,
          name: preset.name,
          source: `github:${preset.repo}`,
          repo_url: `https://github.com/${preset.repo}`,
          transport: "stdio",
          command: preset.command,
          args: preset.args,
          url: null,
          env,
        },
      });
      onChanged(`Added ${preset.name} & synced to .grex-mcp.json`);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleImportJson() {
    if (!jsonInput.trim()) return;
    setError(null);
    setBusy(true);
    try {
      const imported = await invoke<InstalledMcp[]>("import_mcp_json_cmd", {
        rawJson: jsonInput.trim(),
      });
      setJsonInput("");
      setTab("installed");
      onChanged(
        `Imported ${imported.length} MCP server(s) & synced to active worktrees`
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleToggle(mcp: InstalledMcp) {
    await invoke("toggle_mcp_cmd", { id: mcp.id, enabled: !mcp.enabled });
    onChanged(`${mcp.enabled ? "Disabled" : "Enabled"} ${mcp.name}`);
  }

  async function handleRemove(mcp: InstalledMcp) {
    await invoke("remove_mcp_cmd", { id: mcp.id });
    onChanged(`Removed ${mcp.name}`);
  }

  async function handleSync() {
    const count = await invoke<number>("sync_mcps_to_worktrees");
    onChanged(`Synced .grex-mcp.json to ${count} active worktree(s)`);
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal modal-wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Plug size={15} color="#f97316" />
            <div>
              <h3>GitHub MCP Registry</h3>
              <div style={{ fontSize: 11, color: "var(--muted)", marginTop: 1 }}>
                All enabled MCPs are auto-injected into every agent worktree's{" "}
                <code>.grex-mcp.json</code>
              </div>
            </div>
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            <button className="btn" onClick={handleSync}>
              <RefreshCw size={12} /> Sync Worktrees
            </button>
            <button
              className="btn btn-ghost"
              style={{ padding: "0 6px" }}
              onClick={onClose}
            >
              <X size={15} />
            </button>
          </div>
        </div>

        <div
          style={{
            display: "flex",
            gap: 6,
            padding: "8px 16px",
            borderBottom: "1px solid var(--border)",
            background: "rgba(255,255,255,0.015)",
          }}
        >
          <button
            className={`btn ${tab === "github" ? "btn-primary" : "btn-ghost"}`}
            onClick={() => setTab("github")}
          >
            <Download size={13} /> Install from GitHub
          </button>
          <button
            className={`btn ${tab === "installed" ? "btn-primary" : "btn-ghost"}`}
            onClick={() => setTab("installed")}
          >
            <Plug size={13} /> Installed ({mcps.length})
          </button>
          <button
            className={`btn ${tab === "json" ? "btn-primary" : "btn-ghost"}`}
            onClick={() => setTab("json")}
          >
            <Code2 size={13} /> Paste mcpServers JSON
          </button>
        </div>

        <div
          className="modal-body"
          style={{ maxHeight: "64vh", overflowY: "auto" }}
        >
          {error && <div className="error-box">{error}</div>}

          {tab === "github" && (
            <>
              <form
                onSubmit={handleInstallGitHub}
                style={{ display: "flex", flexDirection: "column", gap: 10 }}
              >
                <div className="field">
                  <label>
                    Install any GitHub MCP Repo (auto-clones, builds Node /
                    Python / Rust, and registers)
                  </label>
                  <div className="field-row">
                    <input
                      style={{ flex: 2 }}
                      placeholder="https://github.com/owner/mcp-server or owner/repo"
                      value={repoInput}
                      onChange={(e) => setRepoInput(e.target.value)}
                    />
                    <input
                      style={{ flex: 1 }}
                      placeholder="Optional alias (e.g. linear)"
                      value={customId}
                      onChange={(e) => setCustomId(e.target.value)}
                    />
                    <button
                      type="submit"
                      className="btn btn-primary"
                      disabled={busy || !repoInput.trim()}
                    >
                      {busy ? "Installing..." : "Clone & Install"}
                    </button>
                  </div>
                </div>
                <div className="field">
                  <label>Optional Env Vars (KEY=VALUE per line)</label>
                  <textarea
                    rows={2}
                    placeholder="GITHUB_PERSONAL_ACCESS_TOKEN=ghp_..."
                    value={envText}
                    onChange={(e) => setEnvText(e.target.value)}
                    style={{ fontFamily: "var(--mono)", fontSize: 12 }}
                  />
                </div>
              </form>

              <div
                style={{
                  borderTop: "1px solid var(--border)",
                  paddingTop: 12,
                  marginTop: 4,
                }}
              >
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 600,
                    textTransform: "uppercase",
                    letterSpacing: "0.07em",
                    color: "var(--muted)",
                    marginBottom: 8,
                  }}
                >
                  Popular GitHub MCPs (1-Click Install)
                </div>
                <div
                  style={{
                    display: "grid",
                    gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
                    gap: 8,
                  }}
                >
                  {GITHUB_PRESETS.map((preset) => {
                    const isAdded = installedIds.has(preset.id);
                    return (
                      <div
                        key={preset.id}
                        style={{
                          padding: 10,
                          borderRadius: 8,
                          border: "1px solid var(--border)",
                          background: "var(--surface-2)",
                          display: "flex",
                          flexDirection: "column",
                          justifyContent: "space-between",
                          gap: 8,
                        }}
                      >
                        <div>
                          <div
                            style={{
                              display: "flex",
                              justifyContent: "space-between",
                              alignItems: "center",
                            }}
                          >
                            <strong style={{ fontSize: 12.5 }}>
                              {preset.name}
                            </strong>
                            <span
                              style={{
                                fontFamily: "var(--mono)",
                                fontSize: 10,
                                color: "var(--muted)",
                              }}
                            >
                              {preset.command}
                            </span>
                          </div>
                          <div
                            style={{
                              fontSize: 11.5,
                              color: "var(--muted)",
                              marginTop: 3,
                              lineHeight: 1.4,
                            }}
                          >
                            {preset.description}
                          </div>
                        </div>
                        <button
                          className={`btn ${isAdded ? "" : "btn-primary"}`}
                          style={{ width: "100%", justifyContent: "center" }}
                          disabled={busy}
                          onClick={() => handleQuickPreset(preset)}
                        >
                          {isAdded ? (
                            <>
                              <Check size={12} /> Installed (Re-sync)
                            </>
                          ) : (
                            <>
                              <Download size={12} /> Add MCP
                            </>
                          )}
                        </button>
                      </div>
                    );
                  })}
                </div>
              </div>
            </>
          )}

          {tab === "installed" && (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {mcps.length === 0 ? (
                <div
                  style={{
                    padding: 24,
                    textAlign: "center",
                    color: "var(--muted)",
                    fontSize: 12.5,
                  }}
                >
                  No external MCP servers installed yet. Switch to{" "}
                  <strong>Install from GitHub</strong> or{" "}
                  <strong>Paste mcpServers JSON</strong>.
                </div>
              ) : (
                mcps.map((mcp) => (
                  <div
                    key={mcp.id}
                    style={{
                      padding: 10,
                      borderRadius: 8,
                      border: "1px solid var(--border)",
                      background: "var(--surface-2)",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: 10,
                    }}
                  >
                    <div style={{ minWidth: 0 }}>
                      <div
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 8,
                        }}
                      >
                        <strong style={{ fontFamily: "var(--mono)", fontSize: 12.5 }}>
                          {mcp.id}
                        </strong>
                        <span className="cli-tag">{mcp.source}</span>
                        {mcp.repo_url && (
                          <a
                            href={mcp.repo_url}
                            target="_blank"
                            rel="noreferrer"
                            style={{
                              color: "var(--muted)",
                              display: "inline-flex",
                            }}
                            title={mcp.repo_url}
                          >
                            <ExternalLink size={11} />
                          </a>
                        )}
                      </div>
                      <div
                        style={{
                          fontFamily: "var(--mono)",
                          fontSize: 11,
                          color: "var(--muted)",
                          marginTop: 4,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {mcp.command
                          ? `${mcp.command} ${mcp.args.join(" ")}`
                          : mcp.url}
                      </div>
                    </div>
                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <button
                        className={`btn ${mcp.enabled ? "btn-success" : ""}`}
                        onClick={() => handleToggle(mcp)}
                      >
                        {mcp.enabled ? "Enabled" : "Disabled"}
                      </button>
                      <button
                        className="btn btn-danger"
                        style={{ padding: "0 8px" }}
                        onClick={() => handleRemove(mcp)}
                        title="Remove MCP"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}

          {tab === "json" && (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <div className="field">
                <label>
                  Paste any <code>mcpServers</code> JSON block from a GitHub
                  README
                </label>
                <textarea
                  rows={11}
                  style={{ fontFamily: "var(--mono)", fontSize: 12 }}
                  placeholder={`{\n  "mcpServers": {\n    "playwright": {\n      "command": "npx",\n      "args": ["-y", "@playwright/mcp@latest"]\n    }\n  }\n}`}
                  value={jsonInput}
                  onChange={(e) => setJsonInput(e.target.value)}
                />
              </div>
              <div style={{ display: "flex", justifyContent: "flex-end" }}>
                <button
                  className="btn btn-primary"
                  disabled={busy || !jsonInput.trim()}
                  onClick={handleImportJson}
                >
                  Import &amp; Sync MCPs
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
