import { useEffect, useMemo, useRef, useState } from "react";
import {
  BookOpen,
  FileCode2,
  PanelLeft,
  Plus,
  Search,
  Sparkles,
  Terminal,
} from "lucide-react";
import type { AgentTab, ReviewItem } from "../types";

export interface PaletteAction {
  id: string;
  label: string;
  hint?: string;
  category: "Action" | "Agent Tab" | "Review Queue";
  icon: "plus" | "book" | "sparkles" | "sidebar" | "terminal" | "review";
  run: () => void;
}

interface CommandPaletteProps {
  tabs: AgentTab[];
  reviews: ReviewItem[];
  onClose: () => void;
  onSelectTab: (tabId: string) => void;
  onSelectReview: (review: ReviewItem) => void;
  onNewTab: () => void;
  onOpenCentral: () => void;
  onLoadSample: () => void;
  onToggleSidebar: () => void;
}

export function CommandPalette({
  tabs,
  reviews,
  onClose,
  onSelectTab,
  onSelectReview,
  onNewTab,
  onOpenCentral,
  onLoadSample,
  onToggleSidebar,
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
        id: "new-tab",
        label: "Spawn New Agent Worktree Tab",
        hint: "Ctrl+N",
        category: "Action",
        icon: "plus",
        run: () => {
          onClose();
          onNewTab();
        },
      },
      {
        id: "central-config",
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
        id: "toggle-sidebar",
        label: "Toggle Sidebar",
        hint: "Ctrl+B",
        category: "Action",
        icon: "sidebar",
        run: () => {
          onClose();
          onToggleSidebar();
        },
      },
      {
        id: "sample-review",
        label: "Load Sample 5-Subtab Changelog Review",
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
        category: "Agent Tab",
        icon: "terminal",
        run: () => {
          onSelectTab(tab.id);
          onClose();
        },
      });
    }

    for (const rev of reviews) {
      base.push({
        id: `rev-${rev.id}`,
        label: `Review ${rev.feature_id} #${rev.iteration}`,
        hint: rev.status,
        category: "Review Queue",
        icon: "review",
        run: () => {
          onSelectReview(rev);
          onClose();
        },
      });
    }

    return base;
  }, [
    tabs,
    reviews,
    onClose,
    onNewTab,
    onOpenCentral,
    onLoadSample,
    onToggleSidebar,
    onSelectTab,
    onSelectReview,
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
      case "plus":
        return <Plus size={14} />;
      case "book":
        return <BookOpen size={14} />;
      case "sparkles":
        return <Sparkles size={14} />;
      case "sidebar":
        return <PanelLeft size={14} />;
      case "terminal":
        return <Terminal size={14} />;
      case "review":
        return <FileCode2 size={14} />;
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
            placeholder="Jump to agent tab, review, or factory action..."
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
