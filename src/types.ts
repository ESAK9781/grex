export type AgentState =
  | "Working"
  | "NeedsReview"
  | "Blocked"
  | "Approved"
  | "Archived";

export type AgentCli = "claude" | "codex" | "antigravity" | "shell";

export type PingUrgency = "low" | "normal" | "high" | "critical";

export type ReviewStatus = "Pending" | "ChangesRequested" | "Approved";

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
  tabs: AgentTab[];
  reviews: ReviewItem[];
  central_files: string[];
  installed_mcps: InstalledMcp[];
}

export interface CreateTabRequest {
  cli: AgentCli;
  repo_path: string;
  base_branch: string;
  slug: string;
  initial_prompt: string;
  skip_worktree?: boolean;
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
