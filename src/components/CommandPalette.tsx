import { useEffect, useMemo, useRef, useState } from "react";
import {
  BookOpen,
  FileCode2,
  LayoutList,
  Plug,
  Plus,
  Search,
  Settings,
  Sparkles,
  Terminal,
} from "lucide-react";
import type { AgentTab, MainTab, ReviewItem } from "../types";

interface PaletteAction {
  id: string;
  label: string;
  hint?: string;
  category: "Navigation" | "Action" | "Terminal" | "Changelog";
  icon: "board" | "terminal" | "review" | "plus" | "mcp" | "book" | "settings" | "sparkles";
  run: () => void;
}

interface CommandPaletteProps {
  tabs: AgentTab[];
  reviews: ReviewItem[];
  shortcuts: Record<string, string>;
  onClose: () => void;
  onSwitchMainTab: (t: MainTab) => void;
  onSelectTab: (tabId: string) => void;
  onSelectReview: (reviewId: string) => void;
  onNewIssue: () => void;
  onQuickSpawn: () => void;
  onOpenMcps: () => void;
  onOpenCentral: () => void;
  onOpenSettings: () => void;
  onLoadSample: () => void;
}

export function CommandPalette({
  tabs,
  reviews,
  shortcuts,
  onClose,
  onSwitchMainTab,
  onSelectTab,
  onSelectReview,
  onNewIssue,
  onQuickSpawn,
  onOpenMcps,
  onOpenCentral,
  onOpenSettings,
  onLoadSample,
}: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIdx, setSelectedIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const actions = useMemo<PaletteAction[]>(() => {
    const base: PaletteAction[] = [
      {
        id: "nav-board",
        label: "Go to Board (Management & Specs)",
        hint: shortcuts.tab_board,
        category: "Navigation",
        icon: "board",
        run: () => {
          onSwitchMainTab("board");
          onClose();
        },
      },
      {
        id: "nav-terminals",
        label: "Go to Terminals",
        hint: shortcuts.tab_terminals,
        category: "Navigation",
        icon: "terminal",
        run: () => {
          onSwitchMainTab("terminals");
          onClose();
        },
      },
      {
        id: "nav-changelogs",
        label: "Go to Changelogs (Fullscreen)",
        hint: shortcuts.tab_changelogs,
        category: "Navigation",
        icon: "review",
        run: () => {
          onSwitchMainTab("changelogs");
          onClose();
        },
      },
      {
        id: "new-issue",
        label: "Write New Feature Spec / Issue",
        hint: shortcuts.new_issue,
        category: "Action",
        icon: "plus",
        run: () => {
          onClose();
          onNewIssue();
        },
      },
      {
        id: "quick-terminal",
        label: "Spawn Quick Agent Terminal",
        hint: shortcuts.new_terminal,
        category: "Action",
        icon: "terminal",
        run: () => {
          onClose();
          onQuickSpawn();
        },
      },
      {
        id: "open-mcps",
        label: "Install & Manage GitHub MCP Servers",
        hint: shortcuts.open_mcps,
        category: "Action",
        icon: "mcp",
        run: () => {
          onClose();
          onOpenMcps();
        },
      },
      {
        id: "open-settings",
        label: "Settings & Keyboard Shortcuts",
        hint: shortcuts.open_settings,
        category: "Action",
        icon: "settings",
        run: () => {
          onClose();
          onOpenSettings();
        },
      },
      {
        id: "open-central",
        label: "Edit Central AGENTS.md & Meta-Skills",
        hint: "~/.agent-factory",
        category: "Action",
        icon: "book",
        run: () => {
          onClose();
          onOpenCentral();
        },
      },
      {
        id: "sample-review",
        label: "Load Sample 5-Subtab Changelog",
        hint: "Demo",
        category: "Action",
        icon: "sparkles",
        run: () => {
          onClose();
          onLoadSample();
        },
      },
    ];

    for (const tab of tabs) {
      base.push({
        id: `tab-${tab.id}`,
        label: `${tab.feature_id} (${tab.cli})`,
        hint: `${tab.branch_name} · ${tab.state}`,
        category: "Terminal",
        icon: "terminal",
        run: () => {
          onSelectTab(tab.id);
          onSwitchMainTab("terminals");
          onClose();
        },
      });
    }

    for (const rev of reviews) {
      base.push({
        id: `rev-${rev.id}`,
        label: `Changelog: ${rev.feature_id} #${rev.iteration}`,
        hint: rev.status,
        category: "Changelog",
        icon: "review",
        run: () => {
          onSelectReview(rev.id);
          onSwitchMainTab("changelogs");
          onClose();
        },
      });
    }

    return base;
  }, [
    tabs,
    reviews,
    shortcuts,
    onClose,
    onSwitchMainTab,
    onSelectTab,
    onSelectReview,
    onNewIssue,
    onQuickSpawn,
    onOpenMcps,
    onOpenCentral,
    onOpenSettings,
    onLoadSample,
  ]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return actions;
    return actions.filter(
      (a) =>
        a.label.toLowerCase().includes(q) ||
        (a.hint && a.hint.toLowerCase().includes(q)) ||
        a.category.toLowerCase().includes(q)
    );
  }, [actions, query]);

  useEffect(() => {
    setSelectedIdx(0);
  }, [query]);

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIdx((i) => Math.min(i + 1, Math.max(0, filtered.length - 1)));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIdx((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" && filtered[selectedIdx]) {
      e.preventDefault();
      filtered[selectedIdx].run();
    }
  }

  function renderIcon(kind: PaletteAction["icon"]) {
    switch (kind) {
      case "board":
        return <LayoutList size={14} />;
      case "terminal":
        return <Terminal size={14} />;
      case "review":
        return <FileCode2 size={14} />;
      case "plus":
        return <Plus size={14} />;
      case "mcp":
        return <Plug size={14} />;
      case "book":
        return <BookOpen size={14} />;
      case "settings":
        return <Settings size={14} />;
      case "sparkles":
        return <Sparkles size={14} />;
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="palette"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="palette-input-row">
          <Search size={15} color="var(--muted)" />
          <input
            ref={inputRef}
            placeholder="Jump to board, terminal, changelog, or action..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <kbd>ESC</kbd>
        </div>
        <div className="palette-list">
          {filtered.length === 0 ? (
            <div className="palette-empty">No matching commands</div>
          ) : (
            filtered.map((item, idx) => (
              <button
                key={item.id}
                className={`palette-item ${idx === selectedIdx ? "selected" : ""}`}
                onMouseEnter={() => setSelectedIdx(idx)}
                onClick={item.run}
              >
                <span className="palette-item-left">
                  <span className="palette-icon">{renderIcon(item.icon)}</span>
                  <span>{item.label}</span>
                </span>
                <span className="palette-item-right">
                  <span className="palette-cat">{item.category}</span>
                  {item.hint && <kbd>{item.hint}</kbd>}
                </span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
