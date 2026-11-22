import { FileCode2, Sparkles } from "lucide-react";
import type { ReviewItem } from "../types";
import { ChangelogViewer } from "./ChangelogViewer";

interface ChangelogsViewProps {
  reviews: ReviewItem[];
  activeReviewId: string | null;
  onSelectReview: (reviewId: string) => void;
  onRequestChanges: (reviewId: string, feedback: string) => Promise<void>;
  onApprove: (reviewId: string) => Promise<void>;
  onLoadSample: () => Promise<void>;
}

export function ChangelogsView({
  reviews,
  activeReviewId,
  onSelectReview,
  onRequestChanges,
  onApprove,
  onLoadSample,
}: ChangelogsViewProps) {
  const activeReviews = reviews.filter((r) => r.status === "Pending");
  const inactiveReviews = reviews.filter((r) => r.status !== "Pending");
  const selected =
    reviews.find((r) => r.id === activeReviewId) ??
    activeReviews[0] ??
    inactiveReviews[0] ??
    null;

  return (
    <div className="changelogs-root">
      <aside className="changelog-nav">
        <div className="changelog-nav-section">
          <div className="changelog-nav-header">
            <span>Active</span>
            <span
              className={`count-badge ${activeReviews.length > 0 ? "alert" : ""}`}
            >
              {activeReviews.length}
            </span>
          </div>
          <div className="changelog-nav-list">
            {activeReviews.length === 0 ? (
              <div className="nav-empty">No pending reviews</div>
            ) : (
              activeReviews.map((rev) => (
                <button
                  key={rev.id}
                  className={`changelog-nav-item ${selected?.id === rev.id ? "active" : ""}`}
                  onClick={() => onSelectReview(rev.id)}
                >
                  <span className="state-dot dot-NeedsReview" />
                  <span className="changelog-nav-name">{rev.feature_id}</span>
                  <span className="changelog-iter">#{rev.iteration}</span>
                </button>
              ))
            )}
          </div>
        </div>

        <div className="changelog-nav-section">
          <div className="changelog-nav-header">
            <span>Inactive</span>
            <span className="count-badge">{inactiveReviews.length}</span>
          </div>
          <div className="changelog-nav-list">
            {inactiveReviews.length === 0 ? (
              <div className="nav-empty">No archived reviews</div>
            ) : (
              inactiveReviews.map((rev) => (
                <button
                  key={rev.id}
                  className={`changelog-nav-item ${selected?.id === rev.id ? "active" : ""}`}
                  onClick={() => onSelectReview(rev.id)}
                >
                  <span
                    className={`state-dot ${rev.status === "Approved" ? "dot-Approved" : "dot-Working"}`}
                  />
                  <span className="changelog-nav-name">{rev.feature_id}</span>
                  <span className="changelog-iter">#{rev.iteration}</span>
                </button>
              ))
            )}
          </div>
        </div>
      </aside>

      <div className="changelog-fullscreen">
        {selected ? (
          <ChangelogViewer
            review={selected}
            onClose={() => {}}
            onRequestChanges={onRequestChanges}
            onApprove={onApprove}
          />
        ) : (
          <div className="empty-state">
            <FileCode2 size={34} color="#f97316" />
            <h2>Fullscreen 5-Subtab Changelog Reviewer</h2>
            <p>
              When agents call <code>submit_changelog</code> via MCP, their
              validated HTML changelogs appear in the left navbar for fullscreen
              review and 1-click merge.
            </p>
            <button className="btn btn-primary" onClick={onLoadSample}>
              <Sparkles size={14} /> Load Sample 5-Subtab Changelog
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
