import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  BellRing,
  BookOpen,
  FileCode2,
  GitBranch,
  PanelLeft,
  Plus,
  Search,
  Sparkles,
  Terminal as TerminalIcon,
} from "lucide-react";
import type {
  AgentTab,
  BootstrapState,
  CreateTabRequest,
  PingEventPayload,
  ReviewItem,
} from "./types";
import { TerminalPane } from "./components/TerminalPane";
import { ChangelogViewer } from "./components/ChangelogViewer";
import { NewTabModal } from "./components/NewTabModal";
import { CentralConfigModal } from "./components/CentralConfigModal";
import { CommandPalette } from "./components/CommandPalette";

export function App() {
  const [state, setState] = useState<BootstrapState | null>(null);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [activeReviewId, setActiveReviewId] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [showNewTab, setShowNewTab] = useState(false);
  const [showCentral, setShowCentral] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const notify = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast((cur) => (cur === msg ? null : cur)), 3800);
  }, []);

  const refresh = useCallback(async () => {
    const data = await invoke<BootstrapState>("get_bootstrap_state");
    setState(data);
    setActiveTabId((cur) => {
      if (cur && data.tabs.some((t) => t.id === cur)) return cur;
      const firstActive = data.tabs.find((t) => t.state !== "Archived");
      return firstActive?.id ?? null;
    });
  }, []);

  useEffect(() => {
    refresh().catch((err) => notify(`Init error: ${err}`));

    const unlisteners: Array<() => void> = [];
    listen("factory-updated", () => {
      refresh().catch(() => {});
    }).then((fn) => unlisteners.push(fn));

    listen<ReviewItem>("changelog-submitted", (ev) => {
      notify(
        `Changelog queued: ${ev.payload.feature_id} (Iter #${ev.payload.iteration})`
      );
      setActiveReviewId(ev.payload.id);
    }).then((fn) => unlisteners.push(fn));

    listen<PingEventPayload>("agent-ping", (ev) => {
      notify(
        `[${ev.payload.urgency.toUpperCase()}] ${ev.payload.feature_id ?? "Agent"}: ${ev.payload.message}`
      );
    }).then((fn) => unlisteners.push(fn));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [refresh, notify]);

  const activeTabs = (state?.tabs ?? []).filter((t) => t.state !== "Archived");
  const activeTab = activeTabs.find((t) => t.id === activeTabId) ?? null;
  const reviews = state?.reviews ?? [];
  const pendingCount = reviews.filter((r) => r.status === "Pending").length;
  const activeReview = reviews.find((r) => r.id === activeReviewId) ?? null;

  // Global developer efficiency keyboard shortcuts
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setShowPalette((v) => !v);
      } else if (mod && e.key.toLowerCase() === "n") {
        e.preventDefault();
        setShowNewTab(true);
      } else if (mod && e.key.toLowerCase() === "b") {
        e.preventDefault();
        setSidebarOpen((v) => !v);
      } else if (mod && e.key >= "1" && e.key <= "9") {
        const idx = Number(e.key) - 1;
        if (activeTabs[idx]) {
          e.preventDefault();
          setActiveTabId(activeTabs[idx].id);
        }
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeTabs]);

  async function handleCreateTab(req: CreateTabRequest) {
    const created = await invoke<AgentTab>("create_agent_tab", { req });
    await refresh();
    setActiveTabId(created.id);
    notify(`Spawned ${created.cli} in ${created.branch_name}`);
  }

  async function handleCloseTab(tab: AgentTab) {
    await invoke("close_tab", { tabId: tab.id, cleanupWorktree: false });
    if (activeTabId === tab.id) setActiveTabId(null);
    await refresh();
    notify(`Archived ${tab.feature_id}`);
  }

  async function handleRequestChanges(reviewId: string, feedback: string) {
    await invoke("request_changes", { reviewId, feedback });
    await refresh();
    notify("Feedback piped directly to agent PTY");
  }

  async function handleApprove(reviewId: string) {
    const out = await invoke<string>("approve_changelog", {
      reviewId,
      skipGitMerge: false,
    });
    await refresh();
    notify(out || "Merged & archived worktree");
  }

  async function handleDismissPing(tabId: string) {
    await invoke("dismiss_ping", { tabId });
    await refresh();
  }

  async function handleLoadSample() {
    const review = await invoke<ReviewItem>("submit_sample_changelog");
    await refresh();
    setActiveReviewId(review.id);
    notify("Validated & queued 5-subtab sample changelog");
  }

  function handleCopyMcp() {
    const url = state?.mcp_url ?? "http://127.0.0.1:4739/mcp";
    navigator.clipboard.writeText(url).then(() => notify(`Copied ${url}`));
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="topbar-left">
          <button
            className="btn btn-ghost"
            style={{ padding: "0 6px" }}
            onClick={() => setSidebarOpen((v) => !v)}
            title="Toggle Sidebar (Ctrl+B)"
          >
            <PanelLeft size={15} />
          </button>
          <div className="brand">
            <div className="brand-mark">GX</div>
            <span>Grex</span>
          </div>
          <button
            className="cmd-trigger"
            onClick={() => setShowPalette(true)}
            title="Command Palette (Ctrl+K)"
          >
            <Search size={13} />
            <span>Quick jump...</span>
            <kbd>Ctrl+K</kbd>
          </button>
        </div>

        <div className="topbar-actions">
          <button
            className="mcp-pill"
            onClick={handleCopyMcp}
            title="Click to copy local MCP endpoint"
          >
            <span className="mcp-dot" />
            127.0.0.1:{state?.mcp_port ?? "4739"}
          </button>
          <button className="btn btn-ghost" onClick={handleLoadSample}>
            <Sparkles size={13} />
            Sample Review
          </button>
          <button className="btn btn-ghost" onClick={() => setShowCentral(true)}>
            <BookOpen size={13} />
            AGENTS.md
          </button>
          <button
            className="btn btn-primary"
            onClick={() => setShowNewTab(true)}
            title="New Agent Tab (Ctrl+N)"
          >
            <Plus size={14} />
            New Agent
          </button>
        </div>
      </header>

      <div className="workspace">
        {sidebarOpen && (
          <aside className="sidebar">
            <div className="sidebar-section">
              <div className="sidebar-header">
                <span>Agent Worktrees</span>
                <span className="count-badge">{activeTabs.length}</span>
              </div>
              <div className="sidebar-list">
                {activeTabs.length === 0 ? (
                  <div
                    style={{
                      fontSize: 11.5,
                      color: "var(--muted)",
                      padding: "10px 6px",
                    }}
                  >
                    Press <kbd>Ctrl+N</kbd> to spawn an isolated Git worktree
                    with a native PTY.
                  </div>
                ) : (
                  activeTabs.map((tab, idx) => (
                    <div
                      key={tab.id}
                      className={`tab-card ${tab.id === activeTabId ? "active" : ""} ${tab.state === "Blocked" ? "blocked" : ""}`}
                      onClick={() => setActiveTabId(tab.id)}
                    >
                      <div className="tab-top">
                        <span className="tab-name">{tab.feature_id}</span>
                        <span className={`state-pill state-${tab.state}`}>
                          {tab.state}
                        </span>
                      </div>
                      <div className="tab-meta">
                        <span className="cli-tag">{tab.cli}</span>
                        <GitBranch size={11} />
                        <span
                          style={{
                            flex: 1,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {tab.branch_name}
                        </span>
                        {idx < 9 && <kbd>{idx + 1}</kbd>}
                      </div>
                      {tab.ping_message && (
                        <div className="ping-banner">
                          <span
                            style={{
                              display: "flex",
                              gap: 5,
                              alignItems: "center",
                            }}
                          >
                            <BellRing size={11} />
                            {tab.ping_message}
                          </span>
                          <button
                            className="btn"
                            style={{ height: 20, padding: "0 6px", fontSize: 10 }}
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDismissPing(tab.id);
                            }}
                          >
                            Ack
                          </button>
                        </div>
                      )}
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="sidebar-section">
              <div className="sidebar-header">
                <span>Review Queue</span>
                <span
                  className={`count-badge ${pendingCount > 0 ? "alert" : ""}`}
                >
                  {pendingCount} pending
                </span>
              </div>
              <div className="sidebar-list">
                {reviews.length === 0 ? (
                  <div
                    style={{
                      fontSize: 11.5,
                      color: "var(--muted)",
                      padding: "10px 6px",
                    }}
                  >
                    Waiting for <code>submit_changelog</code> MCP calls.
                  </div>
                ) : (
                  reviews.map((rev) => (
                    <div
                      key={rev.id}
                      className={`tab-card ${rev.id === activeReviewId ? "active" : ""}`}
                      onClick={() => {
                        setActiveReviewId(rev.id);
                        setActiveTabId(rev.tab_id);
                      }}
                    >
                      <div className="tab-top">
                        <span className="tab-name">{rev.feature_id}</span>
                        <span
                          className={`state-pill state-${rev.status === "Pending" ? "NeedsReview" : rev.status === "Approved" ? "Approved" : "Working"}`}
                        >
                          #{rev.iteration} {rev.status}
                        </span>
                      </div>
                      <div className="tab-meta">
                        <FileCode2 size={11} />
                        <span>5-subtab contract verified</span>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          </aside>
        )}

        <main className="main-pane">
          {activeTab ? (
            <TerminalPane tab={activeTab} onCloseTab={handleCloseTab} />
          ) : !activeReview ? (
            <div className="empty-state">
              <TerminalIcon size={34} color="#f97316" />
              <h2>Local Rust Agent Software Factory</h2>
              <p>
                Spawn isolated Git worktrees with native OS PTYs, automatic{" "}
                <code>AGENTS.md</code> &amp; <code>.skills/</code> injection, an
                embedded Axum MCP server, and 5-subtab HTML changelog reviews.
              </p>
              <div style={{ display: "flex", gap: 8, marginTop: 4 }}>
                <button
                  className="btn btn-primary"
                  onClick={() => setShowNewTab(true)}
                >
                  <Plus size={14} /> Spawn Agent <kbd style={{ color: "#090b10", borderColor: "rgba(0,0,0,0.2)" }}>Ctrl+N</kbd>
                </button>
                <button className="btn" onClick={handleLoadSample}>
                  <Sparkles size={13} /> Preview 5-Subtab Reviewer
                </button>
              </div>
            </div>
          ) : null}

          {activeReview && (
            <ChangelogViewer
              review={activeReview}
              onClose={() => setActiveReviewId(null)}
              onRequestChanges={handleRequestChanges}
              onApprove={handleApprove}
            />
          )}
        </main>
      </div>

      <footer className="statusbar">
        <div className="statusbar-hints">
          <span>
            <kbd>Ctrl+K</kbd> Palette
          </span>
          <span>
            <kbd>Ctrl+N</kbd> New Agent
          </span>
          <span>
            <kbd>Ctrl+B</kbd> Sidebar
          </span>
          <span>
            <kbd>Ctrl+1-9</kbd> Switch Tab
          </span>
        </div>
        <div>
          {activeTabs.length} active &middot; {pendingCount} pending review
        </div>
      </footer>

      {showPalette && (
        <CommandPalette
          tabs={activeTabs}
          reviews={reviews}
          onClose={() => setShowPalette(false)}
          onSelectTab={(id) => setActiveTabId(id)}
          onSelectReview={(rev) => {
            setActiveReviewId(rev.id);
            setActiveTabId(rev.tab_id);
          }}
          onNewTab={() => setShowNewTab(true)}
          onOpenCentral={() => setShowCentral(true)}
          onLoadSample={handleLoadSample}
          onToggleSidebar={() => setSidebarOpen((v) => !v)}
        />
      )}

      {showNewTab && (
        <NewTabModal
          onClose={() => setShowNewTab(false)}
          onCreate={handleCreateTab}
        />
      )}

      {showCentral && state && (
        <CentralConfigModal
          centralDir={state.central_dir}
          files={state.central_files}
          onClose={() => setShowCentral(false)}
          onSaved={(msg) => {
            notify(msg);
            refresh();
          }}
        />
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
export default App;
