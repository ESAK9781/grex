use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum AgentState {
    Working,
    Inactive,
    NeedsReview,
    Blocked,
    Approved,
    Archived,
}

impl AgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentState::Working => "Working",
            AgentState::Inactive => "Inactive",
            AgentState::NeedsReview => "NeedsReview",
            AgentState::Blocked => "Blocked",
            AgentState::Approved => "Approved",
            AgentState::Archived => "Archived",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "Inactive" => AgentState::Inactive,
            "NeedsReview" => AgentState::NeedsReview,
            "Blocked" => AgentState::Blocked,
            "Approved" => AgentState::Approved,
            "Archived" => AgentState::Archived,
            _ => AgentState::Working,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AgentPermission {
    #[default]
    Default,
    Auto,
    DangerouslySkip,
}

impl AgentPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentPermission::Default => "default",
            AgentPermission::Auto => "auto",
            AgentPermission::DangerouslySkip => "dangerously-skip",
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
    #[serde(default)]
    pub issue_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum IssueStatus {
    Open,
    InProgress,
    InReview,
    Done,
}

impl IssueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueStatus::Open => "Open",
            IssueStatus::InProgress => "InProgress",
            IssueStatus::InReview => "InReview",
            IssueStatus::Done => "Done",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().replace(['_', '-', ' '], "").as_str() {
            "inprogress" | "working" => IssueStatus::InProgress,
            "inreview" | "needsreview" | "review" => IssueStatus::InReview,
            "done" | "closed" | "approved" | "completed" => IssueStatus::Done,
            _ => IssueStatus::Open,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IssuePriority {
    Low,
    Medium,
    High,
    Critical,
}

impl IssuePriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssuePriority::Low => "low",
            IssuePriority::Medium => "medium",
            IssuePriority::High => "high",
            IssuePriority::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "low" => IssuePriority::Low,
            "high" => IssuePriority::High,
            "critical" => IssuePriority::Critical,
            _ => IssuePriority::Medium,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    pub id: String,
    pub issue_id: String,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub number: i64,
    pub workspace_path: String,
    pub title: String,
    pub spec: String,
    pub status: IssueStatus,
    pub priority: IssuePriority,
    pub labels: Vec<String>,
    pub assigned_tab_id: Option<String>,
    pub branch_name: Option<String>,
    pub comments: Vec<IssueComment>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIssueRequest {
    pub title: String,
    pub spec: String,
    #[serde(default)]
    pub priority: Option<IssuePriority>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIssueRequest {
    pub issue_id: String,
    pub title: Option<String>,
    pub spec: Option<String>,
    pub status: Option<IssueStatus>,
    pub priority: Option<IssuePriority>,
    pub labels: Option<Vec<String>>,
    pub branch_name: Option<String>,
    pub comment: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub path: String,
    pub name: String,
    pub is_git_repo: bool,
    pub base_branch: String,
    pub remote_url: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_inactive_secs() -> u64 {
    25
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_cli: AgentCli,
    #[serde(default)]
    pub default_permission: AgentPermission,
    #[serde(default = "default_true")]
    pub notify_on_inactive: bool,
    #[serde(default = "default_inactive_secs")]
    pub inactive_timeout_secs: u64,
    #[serde(default)]
    pub shortcuts: BTreeMap<String, String>,
    #[serde(default)]
    pub recent_workspaces: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut shortcuts = BTreeMap::new();
        shortcuts.insert("tab_board".into(), "Ctrl+1".into());
        shortcuts.insert("tab_terminals".into(), "Ctrl+2".into());
        shortcuts.insert("tab_changelogs".into(), "Ctrl+3".into());
        shortcuts.insert("command_palette".into(), "Ctrl+K".into());
        shortcuts.insert("new_issue".into(), "Ctrl+I".into());
        shortcuts.insert("new_terminal".into(), "Ctrl+N".into());
        shortcuts.insert("open_mcps".into(), "Ctrl+M".into());
        shortcuts.insert("open_settings".into(), "Ctrl+,".into());
        Self {
            default_cli: AgentCli::Claude,
            default_permission: AgentPermission::Default,
            notify_on_inactive: true,
            inactive_timeout_secs: 25,
            shortcuts,
            recent_workspaces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrexConfigBundle {
    pub version: String,
    pub exported_at: String,
    pub settings: AppSettings,
    pub installed_mcps: Vec<crate::mcps::InstalledMcp>,
    pub central_files: BTreeMap<String, String>,
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
    #[serde(default)]
    pub cli: Option<AgentCli>,
    #[serde(default)]
    pub repo_path: Option<String>,
    #[serde(default)]
    pub base_branch: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub initial_prompt: String,
    #[serde(default)]
    pub skip_worktree: bool,
    #[serde(default)]
    pub issue_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapState {
    pub mcp_port: u16,
    pub mcp_url: String,
    pub central_dir: String,
    pub workspace: Option<WorkspaceInfo>,
    pub settings: AppSettings,
    pub tabs: Vec<AgentTab>,
    pub reviews: Vec<ReviewItem>,
    pub issues: Vec<Issue>,
    pub central_files: Vec<String>,
    pub installed_mcps: Vec<crate::mcps::InstalledMcp>,
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
