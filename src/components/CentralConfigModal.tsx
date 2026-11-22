import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Save, X } from "lucide-react";

interface CentralConfigModalProps {
  centralDir: string;
  files: string[];
  onClose: () => void;
  onSaved: (msg: string) => void;
}

export function CentralConfigModal({
  centralDir,
  files,
  onClose,
  onSaved,
}: CentralConfigModalProps) {
  const [selectedFile, setSelectedFile] = useState(files[0] ?? "AGENTS.md");
  const [content, setContent] = useState("");
  const [commitMsg, setCommitMsg] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!selectedFile) return;
    invoke<string>("read_central_file", { targetFile: selectedFile })
      .then(setContent)
      .catch((err) => setContent(`Error loading file: ${err}`));
  }, [selectedFile]);

  async function handleSave() {
    setSaving(true);
    try {
      const hash = await invoke<string>("save_central_file", {
        targetFile: selectedFile,
        content,
        commitMessage: commitMsg.trim() || null,
      });
      onSaved(`Saved ${selectedFile} atomically (git ${hash})`);
      setCommitMsg("");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal modal-wide"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <h3>Central Factory Instructions &amp; Meta-Skills</h3>
            <div style={{ fontSize: 11.5, color: "var(--muted)", marginTop: 2 }}>
              {centralDir}
            </div>
          </div>
          <button className="btn" style={{ padding: "4px 6px" }} onClick={onClose}>
            <X size={15} />
          </button>
        </div>

        <div className="modal-body">
          <div className="field-row">
            <select
              style={{
                flex: 1,
                background: "#090c12",
                border: "1px solid var(--border)",
                color: "var(--text)",
                borderRadius: 7,
                padding: "7px 10px",
                fontFamily: "var(--mono)",
                fontSize: 12.5,
              }}
              value={selectedFile}
              onChange={(e) => setSelectedFile(e.target.value)}
            >
              {files.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
            <input
              style={{
                flex: 1,
                background: "#090c12",
                border: "1px solid var(--border)",
                color: "var(--text)",
                borderRadius: 7,
                padding: "7px 10px",
                fontSize: 12.5,
              }}
              placeholder="Optional Git commit message..."
              value={commitMsg}
              onChange={(e) => setCommitMsg(e.target.value)}
            />
          </div>

          <textarea
            rows={18}
            style={{
              width: "100%",
              background: "#080a0f",
              border: "1px solid var(--border)",
              borderRadius: 8,
              color: "var(--text)",
              fontFamily: "var(--mono)",
              fontSize: 12.5,
              padding: 12,
              lineHeight: 1.5,
            }}
            value={content}
            onChange={(e) => setContent(e.target.value)}
          />
        </div>

        <div className="modal-footer">
          <button className="btn" onClick={onClose}>
            Close
          </button>
          <button
            className="btn btn-primary"
            disabled={saving}
            onClick={handleSave}
          >
            <Save size={14} />
            {saving ? "Committing..." : "Atomic Save & Git Commit"}
          </button>
        </div>
      </div>
    </div>
  );
}
