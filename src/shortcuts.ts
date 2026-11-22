export const DEFAULT_SHORTCUTS: Record<string, string> = {
  tab_board: "Ctrl+1",
  tab_terminals: "Ctrl+2",
  tab_changelogs: "Ctrl+3",
  command_palette: "Ctrl+K",
  new_issue: "Ctrl+I",
  new_terminal: "Ctrl+N",
  open_mcps: "Ctrl+M",
  open_settings: "Ctrl+,",
};

export const SHORTCUT_LABELS: Record<string, string> = {
  tab_board: "Switch to Board (Management)",
  tab_terminals: "Switch to Terminals",
  tab_changelogs: "Switch to Changelogs",
  command_palette: "Open Command Palette",
  new_issue: "New Feature Spec / Issue",
  new_terminal: "Spawn Quick Agent Terminal",
  open_mcps: "Open GitHub MCP Registry",
  open_settings: "Open Settings & Shortcuts",
};

export interface KeyEventLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export function eventToCombo(e: KeyEventLike): string | null {
  const key = e.key;
  if (
    key === "Control" ||
    key === "Shift" ||
    key === "Alt" ||
    key === "Meta"
  ) {
    return null;
  }
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");

  const normalizedKey =
    key.length === 1 ? key.toUpperCase() : key;
  parts.push(normalizedKey);
  return parts.join("+");
}

export function matchesShortcut(
  e: KeyboardEvent,
  configured: string | undefined
): boolean {
  if (!configured) return false;
  const combo = eventToCombo(e);
  if (!combo) return false;
  return combo.toLowerCase() === configured.trim().toLowerCase();
}
