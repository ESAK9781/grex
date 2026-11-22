import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { CheckCircle2, RotateCcw, X } from "lucide-react";
import type { ReviewItem } from "../types";

interface ChangelogViewerProps {
  review: ReviewItem;
  onClose: () => void;
  onRequestChanges: (reviewId: string, feedback: string) => Promise<void>;
  onApprove: (reviewId: string) => Promise<void>;
}

export function ChangelogViewer({
  review,
  onClose,
  onRequestChanges,
  onApprove,
}: ChangelogViewerProps) {
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);
  const [htmlFallback, setHtmlFallback] = useState<string | null>(null);
  const [useAssetProtocol, setUseAssetProtocol] = useState(true);

  const assetUrl = convertFileSrc(review.html_path);

  useEffect(() => {
    setFeedback("");
    setUseAssetProtocol(true);
    invoke<string>("read_changelog_html", { htmlPath: review.html_path })
      .then((html) => setHtmlFallback(html))
      .catch(() => setHtmlFallback(null));
  }, [review.id, review.html_path]);

  async function handleRequestChanges() {
    if (!feedback.trim()) return;
    setBusy(true);
    try {
      await onRequestChanges(review.id, feedback.trim());
      setFeedback("");
    } finally {
      setBusy(false);
    }
  }

  async function handleApprove() {
    setBusy(true);
    try {
      await onApprove(review.id);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="split-right">
      <div className="pane-header">
        <div>
          <strong>Review: {review.feature_id}</strong> &middot; Iter #
          {review.iteration} &middot;{" "}
          <span className={`state-pill state-${review.status === "Pending" ? "NeedsReview" : review.status === "Approved" ? "Approved" : "Working"}`}>
            {review.status}
          </span>
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          <button
            className="btn"
            style={{ padding: "3px 8px", fontSize: 11 }}
            onClick={() => setUseAssetProtocol((v) => !v)}
            title="Toggle between Tauri asset:// protocol and inline srcDoc"
          >
            {useAssetProtocol ? "asset://" : "srcDoc"}
          </button>
          <button
            className="btn"
            style={{ padding: "3px 6px" }}
            onClick={onClose}
            aria-label="Close changelog viewer"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {useAssetProtocol ? (
        <iframe
          key={`${review.id}-asset`}
          className="changelog-frame"
          sandbox="allow-scripts allow-same-origin"
          src={assetUrl}
          title={`Changelog for ${review.feature_id}`}
        />
      ) : (
        <iframe
          key={`${review.id}-doc`}
          className="changelog-frame"
          sandbox="allow-scripts"
          srcDoc={htmlFallback ?? "<p>Unable to read changelog HTML.</p>"}
          title={`Changelog for ${review.feature_id}`}
        />
      )}

      <div className="review-bar">
        <textarea
          placeholder="Enter review feedback (Ctrl+Enter to send) — writes directly to the agent's PTY terminal..."
          value={feedback}
          onChange={(e) => setFeedback(e.target.value)}
          onKeyDown={(e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
              e.preventDefault();
              handleRequestChanges();
            }
          }}
          disabled={busy || review.status === "Approved"}
        />
        <div className="review-bar-actions">
          <span style={{ fontSize: 11.5, color: "var(--muted)" }}>
            Approve runs <code>git merge --no-ff</code> &amp; cleans up worktree
          </span>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              className="btn"
              disabled={busy || !feedback.trim() || review.status === "Approved"}
              onClick={handleRequestChanges}
            >
              <RotateCcw size={14} />
              Request Changes
            </button>
            <button
              className="btn btn-success"
              disabled={busy || review.status === "Approved"}
              onClick={handleApprove}
            >
              <CheckCircle2 size={14} />
              Approve &amp; Merge
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
