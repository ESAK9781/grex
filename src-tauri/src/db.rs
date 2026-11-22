use crate::models::{
    AgentCli, AgentState, AgentTab, AppSettings, Issue, IssueComment, IssuePriority, IssueStatus,
    PingUrgency, ReviewItem, ReviewStatus,
};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS agent_tabs (
                id TEXT PRIMARY KEY,
                feature_id TEXT NOT NULL UNIQUE,
                slug TEXT NOT NULL,
                cli TEXT NOT NULL,
                repo_path TEXT NOT NULL,
                worktree_path TEXT NOT NULL,
                branch_name TEXT NOT NULL,
                base_branch TEXT NOT NULL,
                state TEXT NOT NULL,
                initial_prompt TEXT NOT NULL DEFAULT '',
                ping_message TEXT,
                ping_urgency TEXT,
                issue_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS review_queue (
                id TEXT PRIMARY KEY,
                tab_id TEXT NOT NULL,
                feature_id TEXT NOT NULL,
                html_path TEXT NOT NULL,
                iteration INTEGER NOT NULL,
                status TEXT NOT NULL,
                feedback TEXT,
                submitted_at TEXT NOT NULL,
                reviewed_at TEXT,
                FOREIGN KEY(tab_id) REFERENCES agent_tabs(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS issues (
                id TEXT PRIMARY KEY,
                number INTEGER NOT NULL,
                workspace_path TEXT NOT NULL,
                title TEXT NOT NULL,
                spec TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
                priority TEXT NOT NULL,
                labels_json TEXT NOT NULL DEFAULT '[]',
                assigned_tab_id TEXT,
                branch_name TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS issue_comments (
                id TEXT PRIMARY KEY,
                issue_id TEXT NOT NULL,
                author TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(issue_id) REFERENCES issues(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_review_feature ON review_queue(feature_id);
            CREATE INDEX IF NOT EXISTS idx_review_status ON review_queue(status);
            CREATE INDEX IF NOT EXISTS idx_issues_workspace ON issues(workspace_path);
            "#,
        )?;

        // Backfill issue_id column on existing databases created prior to v0.2.0
        let _ = conn.execute("ALTER TABLE agent_tabs ADD COLUMN issue_id TEXT", []);
        Ok(())
    }

    pub fn load_settings(&self) -> AppSettings {
        let conn = self.conn.lock().unwrap();
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'settings'",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap_or(None);

        let mut defaults = AppSettings::default();
        if let Some(json) = raw {
            if let Ok(parsed) = serde_json::from_str::<AppSettings>(&json) {
                defaults.default_cli = parsed.default_cli;
                defaults.default_permission = parsed.default_permission;
                defaults.notify_on_inactive = parsed.notify_on_inactive;
                defaults.inactive_timeout_secs = parsed.inactive_timeout_secs;
                defaults.recent_workspaces = parsed.recent_workspaces;
                for (k, v) in parsed.shortcuts {
                    defaults.shortcuts.insert(k, v);
                }
            }
        }
        defaults
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(settings)?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('settings', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json],
        )?;
        Ok(())
    }

    pub fn record_recent_workspace(&self, path: &str) -> Result<AppSettings> {
        let mut settings = self.load_settings();
        settings.recent_workspaces.retain(|p| p != path);
        settings.recent_workspaces.insert(0, path.to_string());
        settings.recent_workspaces.truncate(8);
        self.save_settings(&settings)?;
        Ok(settings)
    }

    pub fn insert_tab(&self, tab: &AgentTab) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO agent_tabs (
                id, feature_id, slug, cli, repo_path, worktree_path,
                branch_name, base_branch, state, initial_prompt,
                ping_message, ping_urgency, issue_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                tab.id,
                tab.feature_id,
                tab.slug,
                tab.cli.as_str(),
                tab.repo_path,
                tab.worktree_path,
                tab.branch_name,
                tab.base_branch,
                tab.state.as_str(),
                tab.initial_prompt,
                tab.ping_message,
                tab.ping_urgency.as_ref().map(PingUrgency::as_str),
                tab.issue_id,
                tab.created_at,
                tab.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_tabs(&self) -> Result<Vec<AgentTab>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, feature_id, slug, cli, repo_path, worktree_path, branch_name, base_branch, state, initial_prompt, ping_message, ping_urgency, issue_id, created_at, updated_at FROM agent_tabs ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_tab)?;
        let mut tabs = Vec::new();
        for row in rows {
            tabs.push(row?);
        }
        Ok(tabs)
    }

    pub fn get_tab_by_id(&self, id: &str) -> Result<Option<AgentTab>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, feature_id, slug, cli, repo_path, worktree_path, branch_name, base_branch, state, initial_prompt, ping_message, ping_urgency, issue_id, created_at, updated_at FROM agent_tabs WHERE id = ?1",
        )?;
        Ok(stmt.query_row(params![id], row_to_tab).optional()?)
    }

    pub fn get_tab_by_feature_id(&self, feature_id: &str) -> Result<Option<AgentTab>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, feature_id, slug, cli, repo_path, worktree_path, branch_name, base_branch, state, initial_prompt, ping_message, ping_urgency, issue_id, created_at, updated_at FROM agent_tabs WHERE feature_id = ?1 OR slug = ?1 LIMIT 1",
        )?;
        Ok(stmt.query_row(params![feature_id], row_to_tab).optional()?)
    }

    pub fn update_tab_state(&self, tab_id: &str, state: AgentState) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE agent_tabs SET state = ?1, updated_at = ?2 WHERE id = ?3",
            params![state.as_str(), now, tab_id],
        )?;
        Ok(())
    }

    pub fn set_tab_ping(
        &self,
        tab_id: &str,
        message: Option<&str>,
        urgency: Option<PingUrgency>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        if let Some(msg) = message {
            conn.execute(
                "UPDATE agent_tabs SET ping_message = ?1, ping_urgency = ?2, state = ?3, updated_at = ?4 WHERE id = ?5",
                params![
                    msg,
                    urgency.as_ref().map(PingUrgency::as_str),
                    AgentState::Blocked.as_str(),
                    now,
                    tab_id
                ],
            )?;
        } else {
            conn.execute(
                "UPDATE agent_tabs SET ping_message = NULL, ping_urgency = NULL, updated_at = ?1 WHERE id = ?2",
                params![now, tab_id],
            )?;
        }
        Ok(())
    }

    pub fn enqueue_review(
        &self,
        tab_id: &str,
        feature_id: &str,
        html_path: &str,
    ) -> Result<ReviewItem> {
        let conn = self.conn.lock().unwrap();
        let prev_iter: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(iteration), 0) FROM review_queue WHERE feature_id = ?1",
                params![feature_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let now = Utc::now().to_rfc3339();
        let item = ReviewItem {
            id: Uuid::new_v4().to_string(),
            tab_id: tab_id.to_string(),
            feature_id: feature_id.to_string(),
            html_path: html_path.to_string(),
            iteration: prev_iter + 1,
            status: ReviewStatus::Pending,
            feedback: None,
            submitted_at: now.clone(),
            reviewed_at: None,
        };

        conn.execute(
            r#"
            INSERT INTO review_queue (
                id, tab_id, feature_id, html_path, iteration, status, feedback, submitted_at, reviewed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                item.id,
                item.tab_id,
                item.feature_id,
                item.html_path,
                item.iteration,
                item.status.as_str(),
                item.feedback,
                item.submitted_at,
                item.reviewed_at,
            ],
        )?;

        conn.execute(
            "UPDATE agent_tabs SET state = ?1, ping_message = NULL, ping_urgency = NULL, updated_at = ?2 WHERE id = ?3",
            params![AgentState::NeedsReview.as_str(), now, tab_id],
        )?;

        Ok(item)
    }

    pub fn list_reviews(&self) -> Result<Vec<ReviewItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tab_id, feature_id, html_path, iteration, status, feedback, submitted_at, reviewed_at FROM review_queue ORDER BY submitted_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_review)?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn get_review(&self, review_id: &str) -> Result<Option<ReviewItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tab_id, feature_id, html_path, iteration, status, feedback, submitted_at, reviewed_at FROM review_queue WHERE id = ?1",
        )?;
        Ok(stmt.query_row(params![review_id], row_to_review).optional()?)
    }

    pub fn mark_review_changes_requested(&self, review_id: &str, feedback: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE review_queue SET status = ?1, feedback = ?2, reviewed_at = ?3 WHERE id = ?4",
            params![
                ReviewStatus::ChangesRequested.as_str(),
                feedback,
                now,
                review_id
            ],
        )?;
        Ok(())
    }

    pub fn mark_review_approved(&self, review_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE review_queue SET status = ?1, reviewed_at = ?2 WHERE id = ?3",
            params![ReviewStatus::Approved.as_str(), now, review_id],
        )?;
        Ok(())
    }

    // --- Issues Board ---

    pub fn create_issue(
        &self,
        workspace_path: &str,
        title: &str,
        spec: &str,
        priority: IssuePriority,
        labels: Vec<String>,
    ) -> Result<Issue> {
        let conn = self.conn.lock().unwrap();
        let next_num: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(number), 0) + 1 FROM issues WHERE workspace_path = ?1",
                params![workspace_path],
                |r| r.get(0),
            )
            .unwrap_or(1);

        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let labels_json = serde_json::to_string(&labels)?;

        conn.execute(
            r#"
            INSERT INTO issues (
                id, number, workspace_path, title, spec, status, priority,
                labels_json, assigned_tab_id, branch_name, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10)
            "#,
            params![
                id,
                next_num,
                workspace_path,
                title,
                spec,
                IssueStatus::Open.as_str(),
                priority.as_str(),
                labels_json,
                now,
                now,
            ],
        )?;

        Ok(Issue {
            id,
            number: next_num,
            workspace_path: workspace_path.to_string(),
            title: title.to_string(),
            spec: spec.to_string(),
            status: IssueStatus::Open,
            priority,
            labels,
            assigned_tab_id: None,
            branch_name: None,
            comments: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn list_issues(&self, workspace_path: Option<&str>) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();
        let mut issues = Vec::new();
        if let Some(ws) = workspace_path {
            let mut stmt = conn.prepare(
                "SELECT id, number, workspace_path, title, spec, status, priority, labels_json, assigned_tab_id, branch_name, created_at, updated_at FROM issues WHERE workspace_path = ?1 ORDER BY number DESC",
            )?;
            let rows = stmt.query_map(params![ws], row_to_issue)?;
            for row in rows {
                issues.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, number, workspace_path, title, spec, status, priority, labels_json, assigned_tab_id, branch_name, created_at, updated_at FROM issues ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map([], row_to_issue)?;
            for row in rows {
                issues.push(row?);
            }
        }

        for issue in &mut issues {
            issue.comments = load_comments_locked(&conn, &issue.id)?;
        }
        Ok(issues)
    }

    pub fn get_issue(&self, id_or_number: &str) -> Result<Option<Issue>> {
        let conn = self.conn.lock().unwrap();
        let mut issue = conn
            .query_row(
                "SELECT id, number, workspace_path, title, spec, status, priority, labels_json, assigned_tab_id, branch_name, created_at, updated_at FROM issues WHERE id = ?1",
                params![id_or_number],
                row_to_issue,
            )
            .optional()?;

        if issue.is_none() {
            if let Ok(num) = id_or_number.trim_start_matches('#').parse::<i64>() {
                issue = conn
                    .query_row(
                        "SELECT id, number, workspace_path, title, spec, status, priority, labels_json, assigned_tab_id, branch_name, created_at, updated_at FROM issues WHERE number = ?1 ORDER BY updated_at DESC LIMIT 1",
                        params![num],
                        row_to_issue,
                    )
                    .optional()?;
            }
        }

        if let Some(ref mut iss) = issue {
            iss.comments = load_comments_locked(&conn, &iss.id)?;
        }
        Ok(issue)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_issue(
        &self,
        issue_id: &str,
        title: Option<&str>,
        spec: Option<&str>,
        status: Option<IssueStatus>,
        priority: Option<IssuePriority>,
        labels: Option<Vec<String>>,
        assigned_tab_id: Option<&str>,
        branch_name: Option<&str>,
        comment: Option<(&str, &str)>,
    ) -> Result<Issue> {
        let Some(mut existing) = self.get_issue(issue_id)? else {
            return Err(anyhow::anyhow!("Issue '{issue_id}' not found"));
        };

        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        if let Some(t) = title {
            existing.title = t.to_string();
        }
        if let Some(s) = spec {
            existing.spec = s.to_string();
        }
        if let Some(st) = status {
            existing.status = st;
        }
        if let Some(p) = priority {
            existing.priority = p;
        }
        if let Some(l) = labels {
            existing.labels = l;
        }
        if let Some(tid) = assigned_tab_id {
            existing.assigned_tab_id = Some(tid.to_string());
        }
        if let Some(br) = branch_name {
            existing.branch_name = Some(br.to_string());
        }
        existing.updated_at = now.clone();

        let labels_json = serde_json::to_string(&existing.labels)?;
        conn.execute(
            r#"
            UPDATE issues
            SET title = ?1, spec = ?2, status = ?3, priority = ?4,
                labels_json = ?5, assigned_tab_id = ?6, branch_name = ?7, updated_at = ?8
            WHERE id = ?9
            "#,
            params![
                existing.title,
                existing.spec,
                existing.status.as_str(),
                existing.priority.as_str(),
                labels_json,
                existing.assigned_tab_id,
                existing.branch_name,
                existing.updated_at,
                existing.id,
            ],
        )?;

        if let Some((author, body)) = comment {
            if !body.trim().is_empty() {
                let c = IssueComment {
                    id: Uuid::new_v4().to_string(),
                    issue_id: existing.id.clone(),
                    author: author.to_string(),
                    body: body.trim().to_string(),
                    created_at: now,
                };
                conn.execute(
                    "INSERT INTO issue_comments (id, issue_id, author, body, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![c.id, c.issue_id, c.author, c.body, c.created_at],
                )?;
                existing.comments.push(c);
            }
        }

        Ok(existing)
    }

    pub fn delete_issue(&self, issue_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM issues WHERE id = ?1", params![issue_id])?;
        Ok(())
    }
}

fn load_comments_locked(conn: &Connection, issue_id: &str) -> Result<Vec<IssueComment>> {
    let mut stmt = conn.prepare(
        "SELECT id, issue_id, author, body, created_at FROM issue_comments WHERE issue_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![issue_id], |r| {
        Ok(IssueComment {
            id: r.get(0)?,
            issue_id: r.get(1)?,
            author: r.get(2)?,
            body: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn row_to_tab(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTab> {
    let cli: String = row.get(3)?;
    let state: String = row.get(8)?;
    let urgency: Option<String> = row.get(11)?;
    Ok(AgentTab {
        id: row.get(0)?,
        feature_id: row.get(1)?,
        slug: row.get(2)?,
        cli: AgentCli::parse(&cli),
        repo_path: row.get(4)?,
        worktree_path: row.get(5)?,
        branch_name: row.get(6)?,
        base_branch: row.get(7)?,
        state: AgentState::parse(&state),
        initial_prompt: row.get(9)?,
        ping_message: row.get(10)?,
        ping_urgency: urgency.map(|u| PingUrgency::parse(&u)),
        issue_id: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn row_to_review(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewItem> {
    let status: String = row.get(5)?;
    Ok(ReviewItem {
        id: row.get(0)?,
        tab_id: row.get(1)?,
        feature_id: row.get(2)?,
        html_path: row.get(3)?,
        iteration: row.get(4)?,
        status: ReviewStatus::parse(&status),
        feedback: row.get(6)?,
        submitted_at: row.get(7)?,
        reviewed_at: row.get(8)?,
    })
}

fn row_to_issue(row: &rusqlite::Row<'_>) -> rusqlite::Result<Issue> {
    let status: String = row.get(5)?;
    let priority: String = row.get(6)?;
    let labels_json: String = row.get(7)?;
    let labels = serde_json::from_str(&labels_json).unwrap_or_default();
    Ok(Issue {
        id: row.get(0)?,
        number: row.get(1)?,
        workspace_path: row.get(2)?,
        title: row.get(3)?,
        spec: row.get(4)?,
        status: IssueStatus::parse(&status),
        priority: IssuePriority::parse(&priority),
        labels,
        assigned_tab_id: row.get(8)?,
        branch_name: row.get(9)?,
        comments: Vec::new(),
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_and_review_lifecycle_persists() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now().to_rfc3339();
        let tab = AgentTab {
            id: "tab-1".into(),
            feature_id: "feat-auth".into(),
            slug: "feat-auth".into(),
            cli: AgentCli::Claude,
            repo_path: "/repo".into(),
            worktree_path: "/repo/.worktrees/feat-auth".into(),
            branch_name: "agent/feat-auth".into(),
            base_branch: "main".into(),
            state: AgentState::Working,
            initial_prompt: "Implement OAuth".into(),
            ping_message: None,
            ping_urgency: None,
            issue_id: None,
            created_at: now.clone(),
            updated_at: now,
        };
        db.insert_tab(&tab).unwrap();

        let rev1 = db
            .enqueue_review("tab-1", "feat-auth", "/tmp/c1.html")
            .unwrap();
        assert_eq!(rev1.iteration, 1);

        let loaded = db.get_tab_by_id("tab-1").unwrap().unwrap();
        assert_eq!(loaded.state, AgentState::NeedsReview);

        db.mark_review_changes_requested(&rev1.id, "Add edge case tests")
            .unwrap();
        let rev2 = db
            .enqueue_review("tab-1", "feat-auth", "/tmp/c2.html")
            .unwrap();
        assert_eq!(rev2.iteration, 2);
    }

    #[test]
    fn issue_board_and_comments_persist() {
        let db = Database::open_in_memory().unwrap();
        let issue = db
            .create_issue(
                "/repo",
                "Add OAuth PKCE",
                "Support S256 PKCE login flow",
                IssuePriority::High,
                vec!["auth".into(), "security".into()],
            )
            .unwrap();
        assert_eq!(issue.number, 1);
        assert_eq!(issue.status, IssueStatus::Open);

        let updated = db
            .update_issue(
                &issue.id,
                None,
                None,
                Some(IssueStatus::InProgress),
                None,
                None,
                Some("tab-1"),
                Some("agent/oauth-pkce"),
                Some(("claude-agent", "Started implementing PKCE verifier")),
            )
            .unwrap();
        assert_eq!(updated.status, IssueStatus::InProgress);
        assert_eq!(updated.comments.len(), 1);
        assert_eq!(updated.branch_name.as_deref(), Some("agent/oauth-pkce"));
    }
}
