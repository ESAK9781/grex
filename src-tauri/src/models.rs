use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum AgentState {
    Working,
    NeedsReview,
    Blocked,
    Approved,
    Archived,
}

impl AgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentState::Working => "Working",
            AgentState::NeedsReview => "NeedsReview",
            AgentState::Blocked => "Blocked",
            AgentState::Approved => "Approved",
            AgentState::Archived => "Archived",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "NeedsReview" => AgentState::NeedsReview,
            "Blocked" => AgentState::Blocked,
            "Approved" => AgentState::Approved,
            "Archived" => AgentState::Archived,
            _ => AgentState::Working,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentCli {
    Claude,
    Codex,
    Antigravity,
    Shell,
}

impl AgentCli {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentCli::Claude => "claude",
            AgentCli::Codex => "codex",
            AgentCli::Antigravity => "antigravity",
            AgentCli::Shell => "shell",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "claude" => AgentCli::Claude,
            "codex" => AgentCli::Codex,
            "antigravity" => AgentCli::Antigravity,
            _ => AgentCli::Shell,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PingUrgency {
    Low,
    Normal,
    High,
    Critical,
}

impl PingUrgency {
    pub fn as_str(&self) -> &'static str {
        match self {
            PingUrgency::Low => "low",
            PingUrgency::Normal => "normal",
            PingUrgency::High => "high",
            PingUrgency::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "low" => PingUrgency::Low,
            "high" => PingUrgency::High,
            "critical" => PingUrgency::Critical,
            _ => PingUrgency::Normal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTab {
    pub id: String,
    pub feature_id: String,
    pub slug: String,
    pub cli: AgentCli,
    pub repo_path: String,
    pub worktree_path: String,
    pub branch_name: String,
    pub base_branch: String,
    pub state: AgentState,
    pub initial_prompt: String,
    pub ping_message: Option<String>,
    pub ping_urgency: Option<PingUrgency>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ReviewStatus {
    Pending,
    ChangesRequested,
    Approved,
}

impl ReviewStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewStatus::Pending => "Pending",
            ReviewStatus::ChangesRequested => "ChangesRequested",
            ReviewStatus::Approved => "Approved",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "ChangesRequested" => ReviewStatus::ChangesRequested,
            "Approved" => ReviewStatus::Approved,
            _ => ReviewStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub tab_id: String,
    pub feature_id: String,
    pub html_path: String,
    pub iteration: i64,
    pub status: ReviewStatus,
    pub feedback: Option<String>,
    pub submitted_at: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeStateInfo {
    pub tab_id: String,
    pub feature_id: String,
    pub slug: String,
    pub branch_name: String,
    pub base_branch: String,
    pub worktree_path: String,
    pub state: AgentState,
    pub touched_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryState {
    pub mcp_port: u16,
    pub central_dir: String,
    pub dev_branch_heads: Vec<RepoHeadInfo>,
    pub active_worktrees: Vec<WorktreeStateInfo>,
    pub pending_reviews: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoHeadInfo {
    pub repo_path: String,
    pub branch: String,
    pub head_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTabRequest {
    pub cli: AgentCli,
    pub repo_path: String,
    pub base_branch: String,
    pub slug: String,
    pub initial_prompt: String,
    #[serde(default)]
    pub skip_worktree: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapState {
    pub mcp_port: u16,
    pub mcp_url: String,
    pub central_dir: String,
    pub tabs: Vec<AgentTab>,
    pub reviews: Vec<ReviewItem>,
    pub central_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyOutputPayload {
    pub tab_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingEventPayload {
    pub tab_id: Option<String>,
    pub feature_id: Option<String>,
    pub message: String,
    pub urgency: PingUrgency,
}
