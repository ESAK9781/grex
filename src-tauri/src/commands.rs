use crate::central::{
    atomic_write, list_central_files, resolve_safe_central_path, update_central_file,
    SAMPLE_CHANGELOG_HTML,
};
use crate::changelog::validate_changelog_file;
use crate::db::Database;
use crate::git::{
    approve_and_merge, create_worktree, detect_default_branch, inject_central_config,
    remove_worktree,
};
use crate::mcp::build_factory_state;
use crate::models::{
    AgentCli, AgentState, AgentTab, BootstrapState, CreateTabRequest, FactoryState, ReviewItem,
};
use crate::pty::{PtyManager, SpawnPtyOptions};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

pub struct AppState {
    pub db: Arc<Database>,
    pub pty: PtyManager,
    pub central_dir: PathBuf,
    pub mcp_port: RwLock<u16>,
}

impl AppState {
    pub fn port(&self) -> u16 {
        *self.mcp_port.read().unwrap()
    }
}

#[tauri::command]
pub async fn get_bootstrap_state(state: State<'_, AppState>) -> Result<BootstrapState, String> {
    let tabs = state.db.list_tabs().map_err(|e| e.to_string())?;
    let reviews = state.db.list_reviews().map_err(|e| e.to_string())?;
    let central_files = list_central_files(&state.central_dir);
    let installed_mcps = crate::mcps::load_registry(&state.central_dir);
    let port = state.port();

    Ok(BootstrapState {
        mcp_port: port,
        mcp_url: format!("http://127.0.0.1:{port}/mcp"),
        central_dir: state.central_dir.to_string_lossy().to_string(),
        tabs,
        reviews,
        central_files,
        installed_mcps,
    })
}

#[tauri::command]
pub async fn detect_git_branch(repo_path: String) -> Result<String, String> {
    Ok(detect_default_branch(Path::new(&repo_path)).await)
}

#[tauri::command]
pub async fn create_agent_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    req: CreateTabRequest,
) -> Result<AgentTab, String> {
    let slug = sanitize_slug(&req.slug);
    if slug.is_empty() {
        return Err("Feature slug cannot be empty".to_string());
    }

    let repo_path = PathBuf::from(&req.repo_path);
    if !repo_path.exists() {
        return Err(format!("Repository path does not exist: {}", req.repo_path));
    }

    let base_branch = if req.base_branch.trim().is_empty() {
        detect_default_branch(&repo_path).await
    } else {
        req.base_branch.trim().to_string()
    };

    let (worktree_path, branch_name) = if req.skip_worktree {
        (repo_path.clone(), base_branch.clone())
    } else {
        create_worktree(&repo_path, &slug, &base_branch)
            .await
            .map_err(|e| e.to_string())?
    };

    let port = state.port();
    let mcp_config_path =
        inject_central_config(&worktree_path, &state.central_dir, port, &slug).ok();

    let now = Utc::now().to_rfc3339();
    let tab = AgentTab {
        id: Uuid::new_v4().to_string(),
        feature_id: slug.clone(),
        slug: slug.clone(),
        cli: req.cli.clone(),
        repo_path: repo_path.to_string_lossy().to_string(),
        worktree_path: worktree_path.to_string_lossy().to_string(),
        branch_name,
        base_branch,
        state: AgentState::Working,
        initial_prompt: req.initial_prompt.clone(),
        ping_message: None,
        ping_urgency: None,
        created_at: now.clone(),
        updated_at: now,
    };

    state.db.insert_tab(&tab).map_err(|e| e.to_string())?;

    state
        .pty
        .spawn_session(
            app.clone(),
            SpawnPtyOptions {
                tab_id: &tab.id,
                feature_id: &tab.feature_id,
                worktree_path: &worktree_path,
                central_dir: &state.central_dir,
                mcp_port: port,
                mcp_config_path: mcp_config_path.as_deref(),
                cli: &tab.cli,
                initial_prompt: &tab.initial_prompt,
                cols: 120,
                rows: 32,
            },
        )
        .map_err(|e| e.to_string())?;

    let _ = app.emit("factory-updated", ());
    Ok(tab)
}

#[tauri::command]
pub async fn ensure_pty_running(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> Result<bool, String> {
    if state.pty.has_session(&tab_id) {
        return Ok(true);
    }
    let Some(tab) = state.db.get_tab_by_id(&tab_id).map_err(|e| e.to_string())? else {
        return Err("Tab not found".to_string());
    };
    let wt = PathBuf::from(&tab.worktree_path);
    if !wt.exists() {
        return Ok(false);
    }
    let mcp_cfg = wt.join(".grex-mcp.json");
    let mcp_cfg_opt = if mcp_cfg.exists() {
        Some(mcp_cfg.as_path())
    } else {
        None
    };

    state
        .pty
        .spawn_session(
            app,
            SpawnPtyOptions {
                tab_id: &tab.id,
                feature_id: &tab.feature_id,
                worktree_path: &wt,
                central_dir: &state.central_dir,
                mcp_port: state.port(),
                mcp_config_path: mcp_cfg_opt,
                cli: &AgentCli::Shell,
                initial_prompt: "",
                cols: 120,
                rows: 32,
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub fn write_pty(state: State<'_, AppState>, tab_id: String, data: String) -> Result<(), String> {
    state.pty.write(&tab_id, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_pty(
    state: State<'_, AppState>,
    tab_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state
        .pty
        .resize(&tab_id, cols, rows)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn close_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
    cleanup_worktree: bool,
) -> Result<(), String> {
    state.pty.close(&tab_id);
    if let Some(tab) = state.db.get_tab_by_id(&tab_id).map_err(|e| e.to_string())? {
        if cleanup_worktree && tab.worktree_path != tab.repo_path {
            let _ = remove_worktree(
                Path::new(&tab.repo_path),
                Path::new(&tab.worktree_path),
                &tab.branch_name,
            )
            .await;
        }
        state
            .db
            .update_tab_state(&tab_id, AgentState::Archived)
            .map_err(|e| e.to_string())?;
    }
    let _ = app.emit("factory-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn request_changes(
    app: AppHandle,
    state: State<'_, AppState>,
    review_id: String,
    feedback: String,
) -> Result<(), String> {
    let review = state
        .db
        .get_review(&review_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Review item not found".to_string())?;

    state
        .db
        .mark_review_changes_requested(&review_id, &feedback)
        .map_err(|e| e.to_string())?;
    state
        .db
        .update_tab_state(&review.tab_id, AgentState::Working)
        .map_err(|e| e.to_string())?;

    // Pipe reviewer feedback directly into the tab's PTY master Write handle
    let instruction = format!(
        "\r\n# [GREX REVIEW FEEDBACK — Iteration #{}]\r\nPlease apply the following requested changes, re-verify tests/UI, regenerate your 5-subtab HTML changelog, and re-call submit_changelog:\r\n{}\r\n",
        review.iteration,
        feedback.trim()
    );
    let _ = state.pty.write(&review.tab_id, &instruction);

    let _ = app.emit("factory-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn approve_changelog(
    app: AppHandle,
    state: State<'_, AppState>,
    review_id: String,
    skip_git_merge: bool,
) -> Result<String, String> {
    let review = state
        .db
        .get_review(&review_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Review item not found".to_string())?;

    let tab = state
        .db
        .get_tab_by_id(&review.tab_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Associated agent tab not found".to_string())?;

    let merge_output = if !skip_git_merge && tab.worktree_path != tab.repo_path {
        state.pty.close(&tab.id);
        approve_and_merge(
            Path::new(&tab.repo_path),
            Path::new(&tab.worktree_path),
            &tab.branch_name,
            &tab.base_branch,
        )
        .await
        .map_err(|e| e.to_string())?
    } else {
        state.pty.close(&tab.id);
        "Approved (no worktree merge required)".to_string()
    };

    state
        .db
        .mark_review_approved(&review_id)
        .map_err(|e| e.to_string())?;
    state
        .db
        .update_tab_state(&tab.id, AgentState::Approved)
        .map_err(|e| e.to_string())?;

    let _ = app.emit("factory-updated", ());
    Ok(merge_output)
}

#[tauri::command]
pub fn dismiss_ping(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> Result<(), String> {
    state
        .db
        .set_tab_ping(&tab_id, None, None)
        .map_err(|e| e.to_string())?;
    state
        .db
        .update_tab_state(&tab_id, AgentState::Working)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("factory-updated", ());
    Ok(())
}

#[tauri::command]
pub fn read_central_file(
    state: State<'_, AppState>,
    target_file: String,
) -> Result<String, String> {
    let full =
        resolve_safe_central_path(&state.central_dir, &target_file).map_err(|e| e.to_string())?;
    std::fs::read_to_string(full).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_central_file(
    app: AppHandle,
    state: State<'_, AppState>,
    target_file: String,
    content: String,
    commit_message: Option<String>,
) -> Result<String, String> {
    let commit = update_central_file(
        &state.central_dir,
        &target_file,
        &content,
        commit_message.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    let _ = app.emit("factory-updated", ());
    Ok(commit)
}

#[tauri::command]
pub fn read_changelog_html(html_path: String) -> Result<String, String> {
    std::fs::read_to_string(&html_path).map_err(|e| format!("Failed to read {html_path}: {e}"))
}

#[tauri::command]
pub async fn get_factory_state_cmd(state: State<'_, AppState>) -> Result<FactoryState, String> {
    build_factory_state(&state.db, &state.central_dir, state.port())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn submit_sample_changelog(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ReviewItem, String> {
    let sample_dir = state.central_dir.join(".samples");
    let sample_path = sample_dir.join("feat-oauth-pkce-changelog.html");
    atomic_write(&sample_path, SAMPLE_CHANGELOG_HTML).map_err(|e| e.to_string())?;

    validate_changelog_file(&sample_path).map_err(|e| e.to_string())?;

    let tab = match state
        .db
        .get_tab_by_feature_id("feat-oauth-pkce")
        .map_err(|e| e.to_string())?
    {
        Some(existing) => existing,
        None => {
            let now = Utc::now().to_rfc3339();
            let demo_tab = AgentTab {
                id: Uuid::new_v4().to_string(),
                feature_id: "feat-oauth-pkce".to_string(),
                slug: "feat-oauth-pkce".to_string(),
                cli: AgentCli::Claude,
                repo_path: state.central_dir.to_string_lossy().to_string(),
                worktree_path: state.central_dir.to_string_lossy().to_string(),
                branch_name: "agent/feat-oauth-pkce".to_string(),
                base_branch: "main".to_string(),
                state: AgentState::NeedsReview,
                initial_prompt: "Implement OAuth 2.1 PKCE login flow".to_string(),
                ping_message: None,
                ping_urgency: None,
                created_at: now.clone(),
                updated_at: now,
            };
            state.db.insert_tab(&demo_tab).map_err(|e| e.to_string())?;
            demo_tab
        }
    };

    let review = state
        .db
        .enqueue_review(
            &tab.id,
            &tab.feature_id,
            &sample_path.to_string_lossy(),
        )
        .map_err(|e| e.to_string())?;

    let _ = app.emit("changelog-submitted", &review);
    let _ = app.emit("factory-updated", ());
    Ok(review)
}

fn sanitize_slug(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[tauri::command]
pub fn list_installed_mcps(
    state: State<'_, AppState>,
) -> Result<Vec<crate::mcps::InstalledMcp>, String> {
    Ok(crate::mcps::load_registry(&state.central_dir))
}

#[tauri::command]
pub async fn install_mcp_from_github(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_url: String,
    custom_id: Option<String>,
    env: std::collections::BTreeMap<String, String>,
) -> Result<crate::mcps::InstalledMcp, String> {
    let installed = crate::mcps::install_from_github(
        &state.central_dir,
        &repo_url,
        custom_id.as_deref(),
        env,
    )
    .await
    .map_err(|e| e.to_string())?;
    let _ = sync_active_worktrees(&state);
    let _ = app.emit("factory-updated", ());
    Ok(installed)
}

#[tauri::command]
pub fn add_custom_mcp(
    app: AppHandle,
    state: State<'_, AppState>,
    req: crate::mcps::AddCustomMcpRequest,
) -> Result<Vec<crate::mcps::InstalledMcp>, String> {
    let id = crate::mcps::sanitize_id(&req.id);
    if id.is_empty() || id == "grex" {
        return Err("Invalid or reserved MCP id".to_string());
    }
    let entry = crate::mcps::InstalledMcp {
        id,
        name: if req.name.trim().is_empty() {
            req.id.clone()
        } else {
            req.name
        },
        source: req.source.unwrap_or_else(|| "custom".to_string()),
        repo_url: req.repo_url,
        transport: req.transport,
        command: req.command,
        args: req.args,
        url: req.url,
        env: req.env,
        enabled: true,
        installed_path: None,
        installed_at: Utc::now().to_rfc3339(),
    };
    let list = crate::mcps::upsert_mcp(&state.central_dir, entry).map_err(|e| e.to_string())?;
    let _ = sync_active_worktrees(&state);
    let _ = app.emit("factory-updated", ());
    Ok(list)
}

#[tauri::command]
pub fn import_mcp_json_cmd(
    app: AppHandle,
    state: State<'_, AppState>,
    raw_json: String,
) -> Result<Vec<crate::mcps::InstalledMcp>, String> {
    let imported =
        crate::mcps::import_mcp_json(&state.central_dir, &raw_json).map_err(|e| e.to_string())?;
    let _ = sync_active_worktrees(&state);
    let _ = app.emit("factory-updated", ());
    Ok(imported)
}

#[tauri::command]
pub fn toggle_mcp_cmd(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<Vec<crate::mcps::InstalledMcp>, String> {
    let list =
        crate::mcps::toggle_mcp(&state.central_dir, &id, enabled).map_err(|e| e.to_string())?;
    let _ = sync_active_worktrees(&state);
    let _ = app.emit("factory-updated", ());
    Ok(list)
}

#[tauri::command]
pub fn update_mcp_env_cmd(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    env: std::collections::BTreeMap<String, String>,
) -> Result<Vec<crate::mcps::InstalledMcp>, String> {
    let list =
        crate::mcps::update_mcp_env(&state.central_dir, &id, env).map_err(|e| e.to_string())?;
    let _ = sync_active_worktrees(&state);
    let _ = app.emit("factory-updated", ());
    Ok(list)
}

#[tauri::command]
pub fn remove_mcp_cmd(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<crate::mcps::InstalledMcp>, String> {
    let list = crate::mcps::remove_mcp(&state.central_dir, &id).map_err(|e| e.to_string())?;
    let _ = sync_active_worktrees(&state);
    let _ = app.emit("factory-updated", ());
    Ok(list)
}

#[tauri::command]
pub fn sync_mcps_to_worktrees(state: State<'_, AppState>) -> Result<usize, String> {
    sync_active_worktrees(&state).map_err(|e| e.to_string())
}

fn sync_active_worktrees(state: &AppState) -> anyhow::Result<usize> {
    let tabs = state.db.list_tabs()?;
    let port = state.port();
    let mut synced = 0usize;
    for tab in tabs {
        if tab.state == AgentState::Archived {
            continue;
        }
        let wt = PathBuf::from(&tab.worktree_path);
        if wt.exists() {
            let _ = inject_central_config(&wt, &state.central_dir, port, &tab.feature_id);
            synced += 1;
        }
    }
    Ok(synced)
}
