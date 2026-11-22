import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Bell,
  Download,
  Keyboard,
  RotateCcw,
  Save,
  Shield,
  Terminal,
  Upload,
  X,
} from "lucide-react";
import {
  DEFAULT_SHORTCUTS,
  SHORTCUT_LABELS,
  eventToCombo,
} from "../shortcuts";
import type { AgentCli, AgentPermission, AppSettings } from "../types";

interface SettingsModalProps {
  settings: AppSettings;
  onClose: () => void;
  onSave: (next: AppSettings) => Promise<void>;
  onConfigTransferred: (msg: string) => void;
}

const PERMISSION_OPTIONS: Array<{
  id: AgentPermission;
  label: string;
  desc: string;
}> = [
  { id: "default", label: "Default", desc: "Ask before edits/commands" },
  { id: "auto", label: "Auto", desc: "acceptEdits / --full-auto" },
  {
    id: "dangerously-skip",
    label: "Dangerously Skip",
    desc: "Bypass all prompts (--dangerously-skip-permissions)",
  },
];

export function SettingsModal({
  settings,
  onClose,
  onSave,
  onConfigTransferred,
}: SettingsModalProps) {
  const [defaultCli, setDefaultCli] = useState<AgentCli>(settings.default_cli);
  const [defaultPermission, setDefaultPermission] = useState<AgentPermission>(
    settings.default_permission ?? "default"
  );
  const [notifyOnInactive, setNotifyOnInactive] = useState<boolean>(
    settings.notify_on_inactive ?? true
  );
  const [inactiveTimeout, setInactiveTimeout] = useState<number>(
    settings.inactive_timeout_secs ?? 25
  );
  const [shortcuts, setShortcuts] = useState<Record<string, string>>({
    ...DEFAULT_SHORTCUTS,
    ...settings.shortcuts,
  });
  const [recordingKey, setRecordingKey] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleSave() {
    setSaving(true);
    try {
      await onSave({
        ...settings,
        default_cli: defaultCli,
        default_permission: defaultPermission,
        notify_on_inactive: notifyOnInactive,
        inactive_timeout_secs: Math.max(10, Number(inactiveTimeout) || 25),
        shortcuts,
      });
      onClose();
    } finally {
      setSaving(false);
    }
  }

  async function handleExport() {
    const dest = await save({
      defaultPath: "grex-config.json",
      filters: [{ name: "Grex Config Bundle", extensions: ["json"] }],
    });
    if (!dest) return;
    await invoke<string>("export_full_config", { destPath: dest });
    onConfigTransferred(`Exported full config to ${dest}`);
  }

  async function handleImport() {
    const src = await open({
      multiple: false,
      filters: [{ name: "Grex Config Bundle", extensions: ["json"] }],
    });
    if (typeof src !== "string") return;
    const summary = await invoke<string>("import_full_config", {
      srcPath: src,
    });
    onConfigTransferred(summary);
    onClose();
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Settings</h3>
          <button
            className="btn btn-ghost"
            style={{ padding: "0 6px" }}
            onClick={onClose}
          >
            <X size={15} />
          </button>
        </div>

        <div
          className="modal-body"
          style={{ maxHeight: "72vh", overflowY: "auto" }}
        >
          <div className="field">
            <label>
              <Terminal size={12} style={{ verticalAlign: -2, marginRight: 5 }} />
              Default Agent CLI
            </label>
            <div className="cli-picker">
              {(["claude", "codex", "antigravity", "shell"] as AgentCli[]).map(
                (cli) => (
                  <button
                    key={cli}
                    className={`btn ${defaultCli === cli ? "btn-primary" : ""}`}
                    onClick={() => setDefaultCli(cli)}
                  >
                    {cli}
                  </button>
                )
              )}
            </div>
          </div>

          <div className="field">
            <label>
              <Shield size={12} style={{ verticalAlign: -2, marginRight: 5 }} />
              Default Agent Permissions
            </label>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(3, 1fr)",
                gap: 6,
              }}
            >
              {PERMISSION_OPTIONS.map((opt) => (
                <button
                  key={opt.id}
                  className={`btn ${defaultPermission === opt.id ? "btn-primary" : ""}`}
                  style={{ justifyContent: "center" }}
                  title={opt.desc}
                  onClick={() => setDefaultPermission(opt.id)}
                >
                  {opt.label}
                </button>
              ))}
            </div>
            <div style={{ fontSize: 11, color: "var(--muted)" }}>
              {
                PERMISSION_OPTIONS.find((p) => p.id === defaultPermission)
                  ?.desc
              }
            </div>
          </div>

          <div
            className="field"
            style={{
              padding: "10px 12px",
              borderRadius: 8,
              border: "1px solid var(--border)",
              background: "var(--surface-2)",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: 10,
              }}
            >
              <label
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  color: "var(--text)",
                  fontSize: 12.5,
                  cursor: "pointer",
                }}
              >
                <input
                  type="checkbox"
                  checked={notifyOnInactive}
                  onChange={(e) => setNotifyOnInactive(e.target.checked)}
                />
                <Bell size={12} color="#f97316" />
                Notify when an agent becomes inactive
              </label>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ fontSize: 11, color: "var(--muted)" }}>
                  Idle (sec):
                </span>
                <input
                  type="number"
                  min={10}
                  max={600}
                  value={inactiveTimeout}
                  onChange={(e) => setInactiveTimeout(Number(e.target.value))}
                  style={{ width: 64, padding: "4px 6px", fontSize: 12 }}
                />
              </div>
            </div>
          </div>

          <div
            style={{
              borderTop: "1px solid var(--border)",
              paddingTop: 12,
            }}
          >
            <div
              style={{
                fontSize: 12,
                fontWeight: 600,
                marginBottom: 8,
              }}
            >
              Portable Full Config (Settings + MCPs + AGENTS.md + Global Skills)
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <button
                className="btn"
                style={{ flex: 1, justifyContent: "center" }}
                onClick={handleExport}
              >
                <Download size={13} /> Export Single File
              </button>
              <button
                className="btn"
                style={{ flex: 1, justifyContent: "center" }}
                onClick={handleImport}
              >
                <Upload size={13} /> Import Config File
              </button>
            </div>
          </div>

          <div
            style={{
              borderTop: "1px solid var(--border)",
              paddingTop: 12,
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <span
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                }}
              >
                <Keyboard size={13} color="#f97316" /> Keyboard Shortcuts
              </span>
              <button
                className="btn btn-ghost"
                style={{ height: 24, fontSize: 11 }}
                onClick={() => setShortcuts({ ...DEFAULT_SHORTCUTS })}
              >
                <RotateCcw size={11} /> Reset
              </button>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
              {Object.keys(DEFAULT_SHORTCUTS).map((actionKey) => {
                const isRecording = recordingKey === actionKey;
                return (
                  <div
                    key={actionKey}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      padding: "6px 10px",
                      borderRadius: 7,
                      border: "1px solid var(--border)",
                      background: "var(--surface-2)",
                    }}
                  >
                    <span style={{ fontSize: 12 }}>
                      {SHORTCUT_LABELS[actionKey] ?? actionKey}
                    </span>
                    <button
                      className={`btn ${isRecording ? "btn-primary" : ""}`}
                      style={{
                        height: 24,
                        minWidth: 100,
                        justifyContent: "center",
                        fontFamily: "var(--mono)",
                        fontSize: 11,
                      }}
                      onClick={() =>
                        setRecordingKey(isRecording ? null : actionKey)
                      }
                      onKeyDown={(e) => {
                        if (!isRecording) return;
                        e.preventDefault();
                        e.stopPropagation();
                        if (e.key === "Escape") {
                          setRecordingKey(null);
                          return;
                        }
                        const combo = eventToCombo(e);
                        if (combo) {
                          setShortcuts((prev) => ({
                            ...prev,
                            [actionKey]: combo,
                          }));
                          setRecordingKey(null);
                        }
                      }}
                    >
                      {isRecording
                        ? "Press keys..."
                        : shortcuts[actionKey] || "Unbound"}
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        <div className="modal-footer">
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn btn-primary"
            disabled={saving}
            onClick={handleSave}
          >
            <Save size={13} />
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
