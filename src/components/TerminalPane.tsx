import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentTab, PtyOutputPayload } from "../types";

interface TerminalPaneProps {
  tab: AgentTab;
  onCloseTab: (tab: AgentTab) => void;
}

export function TerminalPane({ tab, onCloseTab }: TerminalPaneProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily:
        '"JetBrains Mono", "Cascadia Code", "Fira Code", Consolas, monospace',
      theme: {
        background: "#080a0f",
        foreground: "#e7edf5",
        cursor: "#f97316",
        selectionBackground: "rgba(249, 115, 22, 0.3)",
        black: "#111622",
        red: "#ef4444",
        green: "#22c55e",
        yellow: "#eab308",
        blue: "#38bdf8",
        magenta: "#c084fc",
        cyan: "#2dd4bf",
        white: "#e2e8f0",
      },
      scrollback: 5000,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(host);
    fit.fit();
    termRef.current = term;

    term.writeln(
      `\x1b[38;5;208m[grex]\x1b[0m attached to \x1b[1m${tab.feature_id}\x1b[0m (${tab.branch_name})`
    );
    term.writeln(`\x1b[90mworktree: ${tab.worktree_path}\x1b[0m\r\n`);

    // Ensure PTY is running if this tab was restored from SQLite
    invoke("ensure_pty_running", { tabId: tab.id }).catch(() => {});

    const dataDisposable = term.onData((data) => {
      invoke("write_pty", { tabId: tab.id, data }).catch(() => {});
    });

    const resizeObserver = new ResizeObserver(() => {
      try {
        fit.fit();
        invoke("resize_pty", {
          tabId: tab.id,
          cols: term.cols,
          rows: term.rows,
        }).catch(() => {});
      } catch {
        // ignore transient layout errors
      }
    });
    resizeObserver.observe(host);

    let unlisten: (() => void) | undefined;
    listen<PtyOutputPayload>("pty-output", (event) => {
      if (event.payload.tab_id === tab.id) {
        term.write(event.payload.data);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
      resizeObserver.disconnect();
      dataDisposable.dispose();
      term.dispose();
      termRef.current = null;
    };
  }, [tab.id, tab.branch_name, tab.feature_id, tab.worktree_path]);

  return (
    <div className="split-left">
      <div className="pane-header">
        <div>
          <strong>{tab.feature_id}</strong> &middot; {tab.cli} &middot;{" "}
          <span>{tab.branch_name}</span>
        </div>
        <button
          className="btn btn-danger"
          style={{ padding: "3px 8px", fontSize: 11.5 }}
          onClick={() => onCloseTab(tab)}
        >
          Archive Tab
        </button>
      </div>
      <div className="terminal-host" ref={hostRef} />
    </div>
  );
}
