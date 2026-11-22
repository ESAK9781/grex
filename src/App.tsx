import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  BookOpen,
  FileCode2,
  FolderGit2,
  LayoutList,
  Plug,
  Search,
  Settings,
  Terminal as TerminalIcon,
} from "lucide-react";
import type {
  AgentTab,
  AppSettings,
  BootstrapState,
  IssuePriority,
  IssueStatus,
  MainTab,
  PingEventPayload,
  ReviewItem,
} from "./types";
import { DEFAULT_SHORTCUTS, matchesShortcut } from "./shortcuts";
import { WorkspaceSetup } from "./components/WorkspaceSetup";
import { BoardView } from "./components/BoardView";
import { TerminalsView } from "./components/TerminalsView";
import { ChangelogsView } from "./components/ChangelogsView";
import { CentralConfigModal } from "./components/CentralConfigModal";
import { CommandPalette } from "./components/CommandPalette";
import { McpManagerModal } from "./components/McpManagerModal";
import { SettingsModal } from "./components/SettingsModal";

export function App() {
  const [state, setState] = useState<BootstrapState | null>(null);
  const [mainTab, setMainTab] = useState<MainTab>("board");
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [activeReviewId, setActiveReviewId] = useState<string | null>(null);
  const [switchingWorkspace, setSwitchingWorkspace] = useState(false);
  const [newIssueOpen, setNewIssueOpen] = useState(false);
  const [showCentral, setShowCentral] = useState(false);
  const [showMcps, setShowMcps] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const notify = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast((cur) => (cur === msg ? null : cur)), 3600);
  }, []);

  const refresh = useCallback(async () => {
    const data = await invoke<BootstrapState>("get_bootstrap_state");
    setState(data);
    setActiveTabId((cur) => {
      if (cur && data.tabs.some((t) => t.id === cur && t.state !== "Archived")) {
        return cur;
      }
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
        `Changelog ready: ${ev.payload.feature_id} (Iter #${ev.payload.iteration})`
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

  const settings: AppSettings = state?.settings ?? {
    default_cli: "claude",
    default_permission: "default",
    notify_on_inactive: true,
    inactive_timeout_secs: 25,
    shortcuts: DEFAULT_SHORTCUTS,
    recent_workspaces: [],
  };
  const shortcuts = { ...DEFAULT_SHORTCUTS, ...settings.shortcuts };
  const activeTabs = (state?.tabs ?? []).filter((t) => t.state !== "Archived");
  const reviews = state?.reviews ?? [];
  const pendingCount = reviews.filter((r) => r.status === "Pending").length;
  const blockedCount = activeTabs.filter((t) => t.state === "Blocked").length;
  const enabledMcpsCount = (state?.installed_mcps ?? []).filter(
    (m) => m.enabled
  ).length;

  const handleQuickSpawn = useCallback(async () => {
    try {
      const created = await invoke<AgentTab>("create_agent_tab", { req: {} });
      await refresh();
      setActiveTabId(created.id);
      setMainTab("terminals");
      notify(`Spawned ${created.cli} on ${created.branch_name}`);
    } catch (err) {
      notify(String(err));
    }
  }, [refresh, notify]);

  // Configurable global keyboard shortcuts
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (showSettings) return; // Don't hijack while recording shortcuts
      if (matchesShortcut(e, shortcuts.command_palette)) {
        e.preventDefault();
        setShowPalette((v) => !v);
      } else if (matchesShortcut(e, shortcuts.tab_board)) {
        e.preventDefault();
        setMainTab("board");
      } else if (matchesShortcut(e, shortcuts.tab_terminals)) {
        e.preventDefault();
        setMainTab("terminals");
      } else if (matchesShortcut(e, shortcuts.tab_changelogs)) {
        e.preventDefault();
        setMainTab("changelogs");
      } else if (matchesShortcut(e, shortcuts.new_issue)) {
        e.preventDefault();
        setMainTab("board");
        setNewIssueOpen(true);
      } else if (matchesShortcut(e, shortcuts.new_terminal)) {
        e.preventDefault();
        handleQuickSpawn();
      } else if (matchesShortcut(e, shortcuts.open_mcps)) {
        e.preventDefault();
        setShowMcps((v) => !v);
      } else if (matchesShortcut(e, shortcuts.open_settings)) {
        e.preventDefault();
        setShowSettings((v) => !v);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [shortcuts, showSettings, handleQuickSpawn]);

  async function handleSaveSettings(next: AppSettings) {
    await invoke("save_app_settings", { settings: next });
    await refresh();
    notify("Settings & shortcuts saved");
  }

  async function handleCreateIssue(
    title: string,
    spec: string,
    priority: IssuePriority,
    labels: string[]
  ) {
    await invoke("create_issue_cmd", {
      req: { title, spec, priority, labels },
    });
    await refresh();
    notify("Feature spec added to board");
  }

  async function handleUpdateIssue(
    issueId: string,
    patch: { status?: IssueStatus; comment?: string }
  ) {
    await invoke("update_issue_cmd", {
      req: {
        issue_id: issueId,
        status: patch.status ?? null,
        comment: patch.comment ?? null,
        author: "operator",
      },
    });
    await refresh();
  }

  async function handleDeleteIssue(issueId: string) {
    await invoke("delete_issue_cmd", { issueId });
    await refresh();
    notify("Issue deleted");
  }

  async function handleSpawnForIssue(issueId: string) {
    const tab = await invoke<AgentTab>("spawn_agent_for_issue", { issueId });
    await refresh();
    setActiveTabId(tab.id);
    setMainTab("terminals");
    notify(`Spawned ${tab.cli} on ${tab.branch_name}`);
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
    setMainTab("changelogs");
    notify("Loaded 5-subtab sample changelog");
  }

  // Session Workspace Gate: prompt once on startup unless launched with `grex ./`
  if (state && (!state.workspace || switchingWorkspace)) {
    return (
      <WorkspaceSetup
        settings={settings}
        onWorkspaceReady={() => {
          setSwitchingWorkspace(false);
          refresh();
        }}
        onSaveSettings={handleSaveSettings}
      />
    );
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="topbar-left">
          <div className="brand">
            <div className="brand-mark">GX</div>
            <span>Grex</span>
          </div>

          <nav className="main-tabs" aria-label="Primary sections">
            <button
              className={`main-tab ${mainTab === "board" ? "active" : ""}`}
              onClick={() => setMainTab("board")}
              title={`Board (${shortcuts.tab_board})`}
            >
              <LayoutList size={14} />
              <span>Board</span>
              <span className="tab-count">{state?.issues.length ?? 0}</span>
            </button>
            <button
              className={`main-tab ${mainTab === "terminals" ? "active" : ""}`}
              onClick={() => setMainTab("terminals")}
              title={`Terminals (${shortcuts.tab_terminals})`}
            >
              <TerminalIcon size={14} />
              <span>Terminals</span>
              <span className={`tab-count ${blockedCount > 0 ? "danger" : ""}`}>
                {activeTabs.length}
              </span>
            </button>
            <button
              className={`main-tab ${mainTab === "changelogs" ? "active" : ""}`}
              onClick={() => setMainTab("changelogs")}
              title={`Changelogs (${shortcuts.tab_changelogs})`}
            >
              <FileCode2 size={14} />
              <span>Changelogs</span>
              {pendingCount > 0 && (
                <span className="tab-count alert">{pendingCount}</span>
              )}
            </button>
          </nav>
        </div>

        <div className="topbar-actions">
          {state?.workspace && (
            <button
              className="ws-pill"
              onClick={() => setSwitchingWorkspace(true)}
              title={`${state.workspace.path} — Click to switch workspace`}
            >
              <FolderGit2 size={12} />
              <span>{state.workspace.name}</span>
              <span className="ws-branch">{state.workspace.base_branch}</span>
            </button>
          )}

          <button
            className="cmd-trigger"
            onClick={() => setShowPalette(true)}
            title={`Command Palette (${shortcuts.command_palette})`}
          >
            <Search size={12} />
            <kbd>{shortcuts.command_palette}</kbd>
          </button>

          <button
            className="btn btn-ghost"
            onClick={() => setShowMcps(true)}
            title={`GitHub MCPs (${shortcuts.open_mcps})`}
          >
            <Plug size={13} />
            MCPs
            {enabledMcpsCount > 0 && (
              <span className="count-badge alert">{enabledMcpsCount}</span>
            )}
          </button>

          <button
            className="btn btn-ghost"
            onClick={() => setShowCentral(true)}
            title="Central AGENTS.md & Meta-Skills"
          >
            <BookOpen size={13} />
          </button>

          <button
            className="btn btn-ghost"
            onClick={() => setShowSettings(true)}
            title={`Settings & Shortcuts (${shortcuts.open_settings})`}
          >
            <Settings size={14} />
          </button>
        </div>
      </header>

      <main className="workspace">
        {mainTab === "board" && (
          <BoardView
            issues={state?.issues ?? []}
            tabs={state?.tabs ?? []}
            defaultCli={settings.default_cli}
            onCreateIssue={handleCreateIssue}
            onUpdateIssue={handleUpdateIssue}
            onDeleteIssue={handleDeleteIssue}
            onSpawnForIssue={handleSpawnForIssue}
            onOpenTerminal={(tabId) => {
              setActiveTabId(tabId);
              setMainTab("terminals");
            }}
            onDismissPing={handleDismissPing}
            newIssueOpen={newIssueOpen}
            setNewIssueOpen={setNewIssueOpen}
          />
        )}

        {mainTab === "terminals" && (
          <TerminalsView
            tabs={state?.tabs ?? []}
            activeTabId={activeTabId}
            defaultCli={settings.default_cli}
            onSelectTab={setActiveTabId}
            onQuickSpawn={handleQuickSpawn}
            onCloseTab={handleCloseTab}
            onDismissPing={handleDismissPing}
          />
        )}

        {mainTab === "changelogs" && (
          <ChangelogsView
            reviews={reviews}
            activeReviewId={activeReviewId}
            onSelectReview={setActiveReviewId}
            onRequestChanges={handleRequestChanges}
            onApprove={handleApprove}
            onLoadSample={handleLoadSample}
          />
        )}
      </main>

      <footer className="statusbar">
        <div className="statusbar-hints">
          <span>
            <kbd>{shortcuts.tab_board}</kbd> Board
          </span>
          <span>
            <kbd>{shortcuts.tab_terminals}</kbd> Terminals
          </span>
          <span>
            <kbd>{shortcuts.tab_changelogs}</kbd> Changelogs
          </span>
          <span>
            <kbd>{shortcuts.new_issue}</kbd> New Spec
          </span>
          <span>
            <kbd>{shortcuts.new_terminal}</kbd> Quick {settings.default_cli}
          </span>
        </div>
        <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
          <span style={{ color: "#86efac", fontFamily: "var(--mono)" }}>
            MCP :{state?.mcp_port ?? 4739}
          </span>
          <span>
            Default: <code>{settings.default_cli}</code>
          </span>
        </div>
      </footer>

      {showPalette && (
        <CommandPalette
          tabs={activeTabs}
          reviews={reviews}
          shortcuts={shortcuts}
          onClose={() => setShowPalette(false)}
          onSwitchMainTab={setMainTab}
          onSelectTab={setActiveTabId}
          onSelectReview={setActiveReviewId}
          onNewIssue={() => {
            setMainTab("board");
            setNewIssueOpen(true);
          }}
          onQuickSpawn={handleQuickSpawn}
          onOpenMcps={() => setShowMcps(true)}
          onOpenCentral={() => setShowCentral(true)}
          onOpenSettings={() => setShowSettings(true)}
          onLoadSample={handleLoadSample}
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

      {showMcps && (
        <McpManagerModal
          mcps={state?.installed_mcps ?? []}
          onClose={() => setShowMcps(false)}
          onChanged={(msg) => {
            notify(msg);
            refresh();
          }}
        />
      )}

      {showSettings && (
        <SettingsModal
          settings={settings}
          onClose={() => setShowSettings(false)}
          onSave={handleSaveSettings}
          onConfigTransferred={(msg) => {
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
