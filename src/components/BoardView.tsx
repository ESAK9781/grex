import { useMemo, useState } from "react";
import {
  BellRing,
  CheckCircle2,
  CircleDot,
  Clock,
  GitBranch,
  MessageSquare,
  Play,
  Plus,
  Search,
  Send,
  Terminal,
  Trash2,
} from "lucide-react";
import type {
  AgentCli,
  AgentTab,
  Issue,
  IssuePriority,
  IssueStatus,
} from "../types";

interface BoardViewProps {
  issues: Issue[];
  tabs: AgentTab[];
  defaultCli: AgentCli;
  onCreateIssue: (
    title: string,
    spec: string,
    priority: IssuePriority,
    labels: string[]
  ) => Promise<void>;
  onUpdateIssue: (
    issueId: string,
    patch: {
      status?: IssueStatus;
      comment?: string;
    }
  ) => Promise<void>;
  onDeleteIssue: (issueId: string) => Promise<void>;
  onSpawnForIssue: (issueId: string) => Promise<void>;
  onOpenTerminal: (tabId: string) => void;
  onDismissPing: (tabId: string) => Promise<void>;
  newIssueOpen: boolean;
  setNewIssueOpen: (v: boolean) => void;
}

const STATUS_FILTERS: Array<{ id: "All" | IssueStatus; label: string }> = [
  { id: "All", label: "All" },
  { id: "Open", label: "Open" },
  { id: "InProgress", label: "In Progress" },
  { id: "InReview", label: "In Review" },
  { id: "Done", label: "Done" },
];

export function BoardView({
  issues,
  tabs,
  defaultCli,
  onCreateIssue,
  onUpdateIssue,
  onDeleteIssue,
  onSpawnForIssue,
  onOpenTerminal,
  onDismissPing,
  newIssueOpen,
  setNewIssueOpen,
}: BoardViewProps) {
  const [filter, setFilter] = useState<"All" | IssueStatus>("All");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(
    issues[0]?.id ?? null
  );
  const [title, setTitle] = useState("");
  const [spec, setSpec] = useState("");
  const [priority, setPriority] = useState<IssuePriority>("medium");
  const [labelsInput, setLabelsInput] = useState("");
  const [commentText, setCommentText] = useState("");
  const [spawningId, setSpawningId] = useState<string | null>(null);

  const activeAgents = tabs.filter(
    (t) => t.state !== "Archived" && t.state !== "Inactive"
  );
  const inactiveAgents = tabs.filter(
    (t) => t.state === "Inactive" || t.state === "Archived"
  );

  const filteredIssues = useMemo(() => {
    const q = query.trim().toLowerCase();
    return issues.filter((iss) => {
      if (filter !== "All" && iss.status !== filter) return false;
      if (!q) return true;
      return (
        iss.title.toLowerCase().includes(q) ||
        iss.spec.toLowerCase().includes(q) ||
        `#${iss.number}`.includes(q) ||
        iss.labels.some((l) => l.toLowerCase().includes(q))
      );
    });
  }, [issues, filter, query]);

  const selected =
    issues.find((i) => i.id === selectedId) ?? filteredIssues[0] ?? null;

  const linkedTab = selected?.assigned_tab_id
    ? tabs.find((t) => t.id === selected.assigned_tab_id) ?? null
    : null;

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim()) return;
    const labels = labelsInput
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    await onCreateIssue(title.trim(), spec.trim(), priority, labels);
    setTitle("");
    setSpec("");
    setLabelsInput("");
    setPriority("medium");
    setNewIssueOpen(false);
  }

  async function handleSpawn(issueId: string) {
    setSpawningId(issueId);
    try {
      await onSpawnForIssue(issueId);
    } finally {
      setSpawningId(null);
    }
  }

  async function handlePostComment() {
    if (!selected || !commentText.trim()) return;
    await onUpdateIssue(selected.id, { comment: commentText.trim() });
    setCommentText("");
  }

  function statusIcon(st: IssueStatus) {
    switch (st) {
      case "Open":
        return <CircleDot size={14} color="#22c55e" />;
      case "InProgress":
        return <Clock size={14} color="#38bdf8" />;
      case "InReview":
        return <GitBranch size={14} color="#f97316" />;
      case "Done":
        return <CheckCircle2 size={14} color="#a855f7" />;
    }
  }

  return (
    <div className="board-root">
      {/* Minimalist Active & Inactive Agents Strip */}
      {(activeAgents.length > 0 || inactiveAgents.length > 0) && (
        <div className="agents-strip">
          {activeAgents.length > 0 && (
            <>
              <span className="agents-strip-label">Active</span>
              <div className="agents-strip-list">
                {activeAgents.map((tab) => (
                  <div
                    key={tab.id}
                    className={`agent-chip ${tab.state === "Blocked" ? "blocked" : ""}`}
                    onClick={() => onOpenTerminal(tab.id)}
                  >
                    <span className={`state-dot dot-${tab.state}`} />
                    <span className="agent-chip-name">{tab.feature_id}</span>
                    <span className={`state-pill state-${tab.state}`}>
                      {tab.state}
                    </span>
                    {tab.ping_message && (
                      <button
                        className="btn btn-danger"
                        style={{ height: 20, padding: "0 6px", fontSize: 10 }}
                        onClick={(e) => {
                          e.stopPropagation();
                          onDismissPing(tab.id);
                        }}
                        title={tab.ping_message}
                      >
                        <BellRing size={10} /> Ack
                      </button>
                    )}
                  </div>
                ))}
              </div>
            </>
          )}

          {inactiveAgents.length > 0 && (
            <>
              <span className="agents-strip-label" style={{ marginLeft: 8 }}>
                Inactive
              </span>
              <div className="agents-strip-list">
                {inactiveAgents.slice(0, 8).map((tab) => (
                  <div
                    key={tab.id}
                    className="agent-chip inactive"
                    onClick={() => onOpenTerminal(tab.id)}
                    title={`${tab.branch_name} (${tab.state})`}
                  >
                    <span className="state-dot dot-Inactive" />
                    <span className="agent-chip-name">{tab.feature_id}</span>
                    <span className="state-pill state-Inactive">
                      {tab.state}
                    </span>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
      )}

      {/* Main Split: GitHub-Issues List + Detail Panel */}
      <div className="board-split">
        <div className="issues-pane">
          <div className="issues-toolbar">
            <div className="filter-pills">
              {STATUS_FILTERS.map((f) => (
                <button
                  key={f.id}
                  className={`filter-pill ${filter === f.id ? "active" : ""}`}
                  onClick={() => setFilter(f.id)}
                >
                  {f.label}
                </button>
              ))}
            </div>
            <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
              <div className="search-box">
                <Search size={13} color="var(--muted)" />
                <input
                  placeholder="Filter specs..."
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
              </div>
              <button
                className="btn btn-primary"
                onClick={() => setNewIssueOpen(!newIssueOpen)}
              >
                <Plus size={14} /> New Spec
              </button>
            </div>
          </div>

          {newIssueOpen && (
            <form className="issue-composer" onSubmit={handleCreate}>
              <input
                autoFocus
                placeholder="Feature title (e.g. Add OAuth 2.1 PKCE login flow)"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
              />
              <textarea
                rows={4}
                placeholder="Write the feature spec, acceptance criteria, edge cases, and UI requirements..."
                value={spec}
                onChange={(e) => setSpec(e.target.value)}
              />
              <div className="issue-composer-footer">
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <select
                    value={priority}
                    onChange={(e) =>
                      setPriority(e.target.value as IssuePriority)
                    }
                  >
                    <option value="low">Low priority</option>
                    <option value="medium">Medium priority</option>
                    <option value="high">High priority</option>
                    <option value="critical">Critical</option>
                  </select>
                  <input
                    style={{ width: 200 }}
                    placeholder="Labels (comma-separated)"
                    value={labelsInput}
                    onChange={(e) => setLabelsInput(e.target.value)}
                  />
                </div>
                <div style={{ display: "flex", gap: 6 }}>
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => setNewIssueOpen(false)}
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    className="btn btn-primary"
                    disabled={!title.trim()}
                  >
                    Create Issue
                  </button>
                </div>
              </div>
            </form>
          )}

          <div className="issues-list">
            {filteredIssues.length === 0 ? (
              <div className="empty-state" style={{ padding: 40 }}>
                <CircleDot size={28} color="#f97316" />
                <h2>No specs on the board yet</h2>
                <p>
                  Write a feature spec here, then click it to spawn a{" "}
                  <code>{defaultCli}</code> agent in an isolated Git worktree.
                  Agents update the board via MCP just like teammates.
                </p>
                <button
                  className="btn btn-primary"
                  onClick={() => setNewIssueOpen(true)}
                >
                  <Plus size={14} /> Write First Feature Spec
                </button>
              </div>
            ) : (
              filteredIssues.map((iss) => {
                const isSelected = selected?.id === iss.id;
                return (
                  <div
                    key={iss.id}
                    className={`issue-row ${isSelected ? "selected" : ""}`}
                    onClick={() => setSelectedId(iss.id)}
                  >
                    <div className="issue-row-icon">{statusIcon(iss.status)}</div>
                    <div className="issue-row-main">
                      <div className="issue-row-title">
                        <span>{iss.title}</span>
                        <span className="issue-num">#{iss.number}</span>
                        {iss.labels.map((l) => (
                          <span key={l} className="label-pill">
                            {l}
                          </span>
                        ))}
                      </div>
                      <div className="issue-row-meta">
                        <span className={`prio-pill prio-${iss.priority}`}>
                          {iss.priority}
                        </span>
                        <span>{iss.status}</span>
                        {iss.branch_name && (
                          <span className="branch-inline">
                            <GitBranch size={10} /> {iss.branch_name}
                          </span>
                        )}
                        {iss.comments.length > 0 && (
                          <span>
                            <MessageSquare size={10} style={{ verticalAlign: -1 }} />{" "}
                            {iss.comments.length}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="issue-row-actions">
                      {iss.assigned_tab_id ? (
                        <button
                          className="btn"
                          style={{ height: 26, fontSize: 11.5 }}
                          onClick={(e) => {
                            e.stopPropagation();
                            onOpenTerminal(iss.assigned_tab_id!);
                          }}
                        >
                          <Terminal size={12} /> Terminal
                        </button>
                      ) : (
                        <button
                          className="btn btn-primary"
                          style={{ height: 26, fontSize: 11.5 }}
                          disabled={spawningId === iss.id}
                          onClick={(e) => {
                            e.stopPropagation();
                            handleSpawn(iss.id);
                          }}
                        >
                          <Play size={11} />{" "}
                          {spawningId === iss.id ? "Spawning..." : "Spawn"}
                        </button>
                      )}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* Right Detail Panel */}
        {selected && (
          <div className="issue-detail">
            <div className="issue-detail-header">
              <div>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span className="issue-num" style={{ fontSize: 13 }}>
                    #{selected.number}
                  </span>
                  <h3 style={{ margin: 0, fontSize: 15 }}>{selected.title}</h3>
                </div>
                <div className="issue-row-meta" style={{ marginTop: 6 }}>
                  <select
                    className="status-select"
                    value={selected.status}
                    onChange={(e) =>
                      onUpdateIssue(selected.id, {
                        status: e.target.value as IssueStatus,
                      })
                    }
                  >
                    <option value="Open">Open</option>
                    <option value="InProgress">In Progress</option>
                    <option value="InReview">In Review</option>
                    <option value="Done">Done</option>
                  </select>
                  <span className={`prio-pill prio-${selected.priority}`}>
                    {selected.priority}
                  </span>
                  {selected.branch_name && (
                    <span className="branch-inline">
                      <GitBranch size={11} /> {selected.branch_name}
                    </span>
                  )}
                </div>
              </div>

              <div style={{ display: "flex", gap: 6 }}>
                {linkedTab && linkedTab.state !== "Archived" ? (
                  <button
                    className="btn btn-primary"
                    onClick={() => onOpenTerminal(linkedTab.id)}
                  >
                    <Terminal size={13} /> Open Agent Terminal
                  </button>
                ) : (
                  <button
                    className="btn btn-primary"
                    disabled={spawningId === selected.id}
                    onClick={() => handleSpawn(selected.id)}
                  >
                    <Play size={13} />
                    {spawningId === selected.id
                      ? "Spawning..."
                      : `Spawn ${defaultCli}`}
                  </button>
                )}
                <button
                  className="btn btn-danger"
                  style={{ padding: "0 8px" }}
                  title="Delete Issue"
                  onClick={() => onDeleteIssue(selected.id)}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            </div>

            <div className="issue-detail-body">
              <div className="spec-box">
                <div className="spec-box-label">Specification</div>
                <pre className="spec-text">
                  {selected.spec || "No detailed specification provided."}
                </pre>
              </div>

              <div className="timeline-section">
                <div className="spec-box-label">
                  Activity &amp; Agent Updates ({selected.comments.length})
                </div>
                {selected.comments.length === 0 ? (
                  <div
                    style={{
                      fontSize: 12,
                      color: "var(--muted)",
                      padding: "8px 0",
                    }}
                  >
                    No updates yet. Spawn an agent or post a comment below.
                  </div>
                ) : (
                  <div className="timeline-list">
                    {selected.comments.map((c) => (
                      <div key={c.id} className="timeline-item">
                        <div className="timeline-meta">
                          <strong>{c.author}</strong>
                          <span>
                            {new Date(c.created_at).toLocaleTimeString()}
                          </span>
                        </div>
                        <div className="timeline-body">{c.body}</div>
                      </div>
                    ))}
                  </div>
                )}

                <div className="comment-row">
                  <input
                    placeholder="Add a note or instruction to this issue..."
                    value={commentText}
                    onChange={(e) => setCommentText(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handlePostComment();
                    }}
                  />
                  <button
                    className="btn"
                    disabled={!commentText.trim()}
                    onClick={handlePostComment}
                  >
                    <Send size={12} /> Comment
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
