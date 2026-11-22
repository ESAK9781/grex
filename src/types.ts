export type AgentState =
  | "Working"
  | "Inactive"
  | "NeedsReview"
  | "Blocked"
  | "Approved"
  | "Archived";

export type AgentPermission = "default" | "auto" | "dangerously-skip";

export type AgentCli = "claude" | "codex" | "antigravity" | "shell";

export type PingUrgency = "low" | "normal" | "high" | "critical";

export type ReviewStatus = "Pending" | "ChangesRequested" | "Approved";

export type IssueStatus = "Open" | "InProgress" | "InReview" | "Done";

export type IssuePriority = "low" | "medium" | "high" | "critical";

export type MainTab = "board" | "terminals" | "changelogs";

export interface AgentTab {
  id: string;
  feature_id: string;
  slug: string;
  cli: AgentCli;
  repo_path: string;
  worktree_path: string;
  branch_name: string;
  base_branch: string;
  state: AgentState;
  initial_prompt: string;
  ping_message: string | null;
  ping_urgency: PingUrgency | null;
  issue_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface ReviewItem {
  id: string;
  tab_id: string;
  feature_id: string;
  html_path: string;
  iteration: number;
  status: ReviewStatus;
  feedback: string | null;
  submitted_at: string;
  reviewed_at: string | null;
}

export interface IssueComment {
  id: string;
  issue_id: string;
  author: string;
  body: string;
  created_at: string;
}

export interface Issue {
  id: string;
  number: number;
  workspace_path: string;
  title: string;
  spec: string;
  status: IssueStatus;
  priority: IssuePriority;
  labels: string[];
  assigned_tab_id: string | null;
  branch_name: string | null;
  comments: IssueComment[];
  created_at: string;
  updated_at: string;
}

export interface WorkspaceInfo {
  path: string;
  name: string;
  is_git_repo: boolean;
  base_branch: string;
  remote_url: string | null;
}

export interface AppSettings {
  default_cli: AgentCli;
  default_permission: AgentPermission;
  notify_on_inactive: boolean;
  inactive_timeout_secs: number;
  shortcuts: Record<string, string>;
  recent_workspaces: string[];
}

export interface InstalledMcp {
  id: string;
  name: string;
  source: string;
  repo_url: string | null;
  transport: string;
  command: string;
  args: string[];
  url: string | null;
  env: Record<string, string>;
  enabled: boolean;
  installed_path: string | null;
  installed_at: string;
}

export interface BootstrapState {
  mcp_port: number;
  mcp_url: string;
  central_dir: string;
  workspace: WorkspaceInfo | null;
  settings: AppSettings;
  tabs: AgentTab[];
  reviews: ReviewItem[];
  issues: Issue[];
  central_files: string[];
  installed_mcps: InstalledMcp[];
}

export interface CreateTabRequest {
  cli?: AgentCli;
  repo_path?: string;
  base_branch?: string;
  slug?: string;
  initial_prompt?: string;
  skip_worktree?: boolean;
  issue_id?: string;
}

export interface PtyOutputPayload {
  tab_id: string;
  data: string;
}

export interface PingEventPayload {
  tab_id: string | null;
  feature_id: string | null;
  message: string;
  urgency: PingUrgency;
}
