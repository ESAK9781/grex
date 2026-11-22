use crate::models::{AgentCli, AgentState, AgentTab, PingUrgency, ReviewItem, ReviewStatus};
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

            CREATE INDEX IF NOT EXISTS idx_review_feature ON review_queue(feature_id);
            CREATE INDEX IF NOT EXISTS idx_review_status ON review_queue(status);
            "#,
        )?;
        Ok(())
    }

    pub fn insert_tab(&self, tab: &AgentTab) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO agent_tabs (
                id, feature_id, slug, cli, repo_path, worktree_path,
                branch_name, base_branch, state, initial_prompt,
                ping_message, ping_urgency, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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
                tab.created_at,
                tab.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_tabs(&self) -> Result<Vec<AgentTab>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, feature_id, slug, cli, repo_path, worktree_path, branch_name, base_branch, state, initial_prompt, ping_message, ping_urgency, created_at, updated_at FROM agent_tabs ORDER BY created_at DESC",
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
            "SELECT id, feature_id, slug, cli, repo_path, worktree_path, branch_name, base_branch, state, initial_prompt, ping_message, ping_urgency, created_at, updated_at FROM agent_tabs WHERE id = ?1",
        )?;
        Ok(stmt.query_row(params![id], row_to_tab).optional()?)
    }

    pub fn get_tab_by_feature_id(&self, feature_id: &str) -> Result<Option<AgentTab>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, feature_id, slug, cli, repo_path, worktree_path, branch_name, base_branch, state, initial_prompt, ping_message, ping_urgency, created_at, updated_at FROM agent_tabs WHERE feature_id = ?1 OR slug = ?1 LIMIT 1",
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
        let new_state = if message.is_some() {
            Some(AgentState::Blocked.as_str())
        } else {
            None
        };
        if let Some(st) = new_state {
            conn.execute(
                "UPDATE agent_tabs SET ping_message = ?1, ping_urgency = ?2, state = ?3, updated_at = ?4 WHERE id = ?5",
                params![message, urgency.as_ref().map(PingUrgency::as_str), st, now, tab_id],
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
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
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
}
