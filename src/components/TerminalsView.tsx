import { BellRing, Plus, Terminal as TerminalIcon, X } from "lucide-react";
import type { AgentCli, AgentTab } from "../types";
import { TerminalPane } from "./TerminalPane";

interface TerminalsViewProps {
  tabs: AgentTab[];
  activeTabId: string | null;
  defaultCli: AgentCli;
  onSelectTab: (tabId: string) => void;
  onQuickSpawn: () => Promise<void>;
  onCloseTab: (tab: AgentTab) => Promise<void>;
  onDismissPing: (tabId: string) => Promise<void>;
}

export function TerminalsView({
  tabs,
  activeTabId,
  defaultCli,
  onSelectTab,
  onQuickSpawn,
  onCloseTab,
  onDismissPing,
}: TerminalsViewProps) {
  const activeTabs = tabs.filter((t) => t.state !== "Archived");
  const current =
    activeTabs.find((t) => t.id === activeTabId) ?? activeTabs[0] ?? null;

  return (
    <div className="terminals-root">
      <div className="subtab-bar">
        <div className="subtab-list">
          {activeTabs.map((tab) => {
            const isSelected = current?.id === tab.id;
            return (
              <div
                key={tab.id}
                className={`subtab ${isSelected ? "active" : ""} ${tab.state === "Blocked" ? "blocked" : ""}`}
                onClick={() => onSelectTab(tab.id)}
              >
                <span className={`state-dot dot-${tab.state}`} />
                <span className="subtab-name">{tab.feature_id}</span>
                <span className="cli-tag">{tab.cli}</span>
                {tab.ping_message && (
                  <button
                    className="subtab-ack"
                    title={tab.ping_message}
                    onClick={(e) => {
                      e.stopPropagation();
                      onDismissPing(tab.id);
                    }}
                  >
                    <BellRing size={11} />
                  </button>
                )}
                <button
                  className="subtab-close"
                  title="Archive terminal tab"
                  onClick={(e) => {
                    e.stopPropagation();
                    onCloseTab(tab);
                  }}
                >
                  <X size={12} />
                </button>
              </div>
            );
          })}
        </div>

        <button
          className="btn btn-primary"
          style={{ height: 28, flexShrink: 0 }}
          onClick={onQuickSpawn}
          title={`Spawn new ${defaultCli} terminal in an isolated worktree`}
        >
          <Plus size={13} /> New {defaultCli}
        </button>
      </div>

      {current ? (
        <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
          <TerminalPane tab={current} onCloseTab={onCloseTab} />
        </div>
      ) : (
        <div className="empty-state">
          <TerminalIcon size={34} color="#f97316" />
          <h2>No active agent terminals</h2>
          <p>
            Click an issue in the <strong>Board</strong> tab to spawn an agent,
            or launch a quick <code>{defaultCli}</code> worktree terminal right
            here.
          </p>
          <button className="btn btn-primary" onClick={onQuickSpawn}>
            <Plus size={14} /> Spawn {defaultCli} Terminal
          </button>
        </div>
      )}
    </div>
  );
}
