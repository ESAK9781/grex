use crate::central::update_central_file;
use crate::changelog::validate_changelog_file;
use crate::db::Database;
use crate::git::{get_head_commit, get_touched_files};
use crate::models::{
    AgentState, FactoryState, IssuePriority, IssueStatus, PingEventPayload, PingUrgency,
    RepoHeadInfo, ReviewStatus, WorktreeStateInfo,
};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct McpContext {
    pub app: AppHandle,
    pub db: Arc<Database>,
    pub central_dir: PathBuf,
    pub port: u16,
    pub sse_tx: broadcast::Sender<String>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

pub async fn start_mcp_server(
    app: AppHandle,
    db: Arc<Database>,
    central_dir: PathBuf,
) -> anyhow::Result<u16> {
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:4739").await {
        Ok(l) => l,
        Err(_) => tokio::net::TcpListener::bind("127.0.0.1:0").await?,
    };
    let addr: SocketAddr = listener.local_addr()?;
    let port = addr.port();
    let (sse_tx, _) = broadcast::channel::<String>(128);

    let ctx = McpContext {
        app,
        db,
        central_dir,
        port,
        sse_tx,
    };

    let router = Router::new()
        .route("/health", get(health_handler))
        .route("/mcp", post(mcp_rpc_handler))
        .route("/messages", post(mcp_rpc_handler))
        .route("/sse", get(sse_handler))
        .layer(CorsLayer::permissive())
        .with_state(ctx);

    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!("MCP server exited with error: {err}");
        }
    });

    Ok(port)
}

async fn health_handler(State(ctx): State<McpContext>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "grex-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        "port": ctx.port,
    }))
}

async fn sse_handler(
    State(ctx): State<McpContext>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = ctx.sse_tx.subscribe();
    let endpoint = format!("http://127.0.0.1:{}/messages", ctx.port);

    let stream = async_stream::stream! {
        yield Ok(Event::default().event("endpoint").data(endpoint));
        loop {
            match rx.recv().await {
                Ok(msg) => yield Ok(Event::default().event("message").data(msg)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn mcp_rpc_handler(
    State(ctx): State<McpContext>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let id = req.id.clone().unwrap_or(Value::Null);
    let response = match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "grex-factory-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        },
        "notifications/initialized" | "ping" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!({})),
            error: None,
        },
        "tools/list" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!({ "tools": mcp_tool_schemas() })),
            error: None,
        },
        "tools/call" => {
            let tool_name = req
                .params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            match dispatch_tool_call(&ctx, tool_name, args).await {
                Ok(text) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "content": [{ "type": "text", "text": text }],
                        "isError": false
                    })),
                    error: None,
                },
                Err(err) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "content": [{ "type": "text", "text": err.to_string() }],
                        "isError": true
                    })),
                    error: None,
                },
            }
        }
        other => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {other}"),
            }),
        },
    };

    if let Ok(serialized) = serde_json::to_string(&response) {
        let _ = ctx.sse_tx.send(serialized);
    }

    Json(response)
}

pub fn mcp_tool_schemas() -> Value {
    json!([
        {
            "name": "submit_changelog",
            "description": "Validates the 5-subtab HTML changelog (Summary, Architecture, UI Before/After, User Stories, Test Verification), pushes it onto the Grex Active Review Queue, and fires a desktop notification.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "feature_id": { "type": "string", "description": "Feature ID / slug of the agent tab" },
                    "html_path": { "type": "string", "description": "Absolute path to the self-contained HTML changelog file" }
                },
                "required": ["feature_id", "html_path"]
            }
        },
        {
            "name": "ping_user",
            "description": "Highlights the agent's tab in Grex and sends a native OS alert when the agent is blocked or needs operator input.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Clear explanation of what you need from the user" },
                    "urgency": { "type": "string", "enum": ["low", "normal", "high", "critical"], "default": "normal" },
                    "feature_id": { "type": "string", "description": "Optional feature ID to highlight a specific tab" }
                },
                "required": ["message"]
            }
        },
        {
            "name": "get_factory_state",
            "description": "Returns active worktrees, current dev branch HEAD commits, agent states, and files touched by other agents to prevent merge conflicts.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "update_central_instructions",
            "description": "Safely updates ~/.agent-factory/AGENTS.md or shared .skills/ files with atomic writes and Git commit history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target_file": { "type": "string", "description": "Relative path inside ~/.agent-factory (e.g. AGENTS.md or .skills/my-skill/SKILL.md)" },
                    "content": { "type": "string", "description": "Full file contents" },
                    "commit_message": { "type": "string", "description": "Optional Git commit message" }
                },
                "required": ["target_file", "content"]
            }
        },
        {
            "name": "list_issues",
            "description": "Lists feature specs and issues from the Grex management board.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["Open", "InProgress", "InReview", "Done"], "description": "Optional status filter" }
                }
            }
        },
        {
            "name": "update_issue",
            "description": "Updates an issue on the Grex board (status, progress comment, branch_name). Also supports appending to the issue spec (append_spec), adding labels (add_labels), or changing priority/title — ONLY modify specs/labels/priority/title when explicitly instructed by the user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "issue_id": { "type": "string", "description": "Issue UUID or number (e.g. '1' or '#1')" },
                    "status": { "type": "string", "enum": ["Open", "InProgress", "InReview", "Done"] },
                    "comment": { "type": "string", "description": "Progress update or note to post on the issue timeline" },
                    "branch_name": { "type": "string", "description": "Feature branch name chosen by the agent" },
                    "append_spec": { "type": "string", "description": "Markdown text to append to the issue spec (ONLY use when explicitly instructed by the user)" },
                    "add_labels": { "type": "array", "items": { "type": "string" }, "description": "Labels to add to the issue (ONLY use when explicitly instructed by the user)" },
                    "priority": { "type": "string", "enum": ["low", "medium", "high", "critical"], "description": "Updated priority (ONLY use when explicitly instructed by the user)" },
                    "title": { "type": "string", "description": "Updated title (ONLY use when explicitly instructed by the user)" },
                    "author": { "type": "string", "description": "Agent name/handle (defaults to 'agent')" }
                },
                "required": ["issue_id"]
            }
        },
        {
            "name": "create_issue",
            "description": "Creates a new issue/spec on the Grex issue board. ONLY call this tool when explicitly instructed by the user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "spec": { "type": "string" },
                    "priority": { "type": "string", "enum": ["low", "medium", "high", "critical"], "default": "medium" },
                    "labels": { "type": "array", "items": { "type": "string" } },
                    "workspace_path": { "type": "string", "description": "Optional workspace path" }
                },
                "required": ["title", "spec"]
            }
        }
    ])
}

async fn dispatch_tool_call(ctx: &McpContext, name: &str, args: Value) -> anyhow::Result<String> {
    match name {
        "submit_changelog" => {
            let feature_id = args
                .get("feature_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing required argument: feature_id"))?;
            let html_path = args
                .get("html_path")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing required argument: html_path"))?;

            let validation = validate_changelog_file(Path::new(html_path))?;

            let tab = ctx
                .db
                .get_tab_by_feature_id(feature_id)?
                .ok_or_else(|| anyhow::anyhow!("No agent tab found for feature_id '{feature_id}'"))?;

            let review = ctx.db.enqueue_review(&tab.id, &tab.feature_id, html_path)?;

            if let Some(ref issue_id) = tab.issue_id {
                let _ = ctx.db.update_issue(
                    issue_id,
                    None,
                    None,
                    Some(IssueStatus::InReview),
                    None,
                    None,
                    Some(&tab.id),
                    Some(&tab.branch_name),
                    Some((
                        tab.cli.as_str(),
                        &format!(
                            "Submitted 5-subtab changelog (Iteration #{}) for review.",
                            review.iteration
                        ),
                    )),
                );
            }

            let _ = ctx.app.emit("changelog-submitted", &review);
            let _ = ctx.app.emit("factory-updated", ());

            let _ = ctx
                .app
                .notification()
                .builder()
                .title(&format!("Grex Review Ready: {}", tab.feature_id))
                .body(&format!(
                    "Iteration #{} validated ({} subtabs) and queued for review.",
                    review.iteration,
                    validation.found_subtabs.len()
                ))
                .show();

            Ok(serde_json::to_string_pretty(&json!({
                "status": "queued",
                "review_id": review.id,
                "feature_id": tab.feature_id,
                "iteration": review.iteration,
                "validated_subtabs": validation.found_subtabs,
                "bytes": validation.byte_len
            }))?)
        }
        "ping_user" => {
            let message = args
                .get("message")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing required argument: message"))?;
            let urgency = args
                .get("urgency")
                .and_then(Value::as_str)
                .map(PingUrgency::parse)
                .unwrap_or(PingUrgency::Normal);
            let feature_id = args.get("feature_id").and_then(Value::as_str);

            let mut resolved_tab_id = None;
            if let Some(fid) = feature_id {
                if let Some(tab) = ctx.db.get_tab_by_feature_id(fid)? {
                    ctx.db.set_tab_ping(&tab.id, Some(message), Some(urgency.clone()))?;
                    resolved_tab_id = Some(tab.id);
                }
            }

            let payload = PingEventPayload {
                tab_id: resolved_tab_id,
                feature_id: feature_id.map(str::to_string),
                message: message.to_string(),
                urgency: urgency.clone(),
            };
            let _ = ctx.app.emit("agent-ping", &payload);
            let _ = ctx.app.emit("factory-updated", ());

            let title = match feature_id {
                Some(fid) => format!("Grex Alert [{}] — {}", urgency.as_str().to_uppercase(), fid),
                None => format!("Grex Agent Alert [{}]", urgency.as_str().to_uppercase()),
            };
            let _ = ctx
                .app
                .notification()
                .builder()
                .title(&title)
                .body(message)
                .show();

            Ok(json!({ "status": "notified", "urgency": urgency.as_str() }).to_string())
        }
        "get_factory_state" => {
            let state = build_factory_state(&ctx.db, &ctx.central_dir, ctx.port).await?;
            Ok(serde_json::to_string_pretty(&state)?)
        }
        "update_central_instructions" => {
            let target_file = args
                .get("target_file")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing required argument: target_file"))?;
            let content = args
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing required argument: content"))?;
            let commit_message = args.get("commit_message").and_then(Value::as_str);

            let commit = update_central_file(&ctx.central_dir, target_file, content, commit_message)?;
            let _ = ctx.app.emit("factory-updated", ());

            Ok(json!({
                "status": "updated",
                "target_file": target_file,
                "commit": commit
            })
            .to_string())
        }
        "list_issues" => {
            let status_filter = args
                .get("status")
                .and_then(Value::as_str)
                .map(IssueStatus::parse);
            let mut issues = ctx.db.list_issues(None)?;
            if let Some(st) = status_filter {
                issues.retain(|i| i.status == st);
            }
            Ok(serde_json::to_string_pretty(&issues)?)
        }
        "update_issue" => {
            let issue_id = args
                .get("issue_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing required argument: issue_id"))?;
            let status = args
                .get("status")
                .and_then(Value::as_str)
                .map(IssueStatus::parse);
            let priority = args
                .get("priority")
                .and_then(Value::as_str)
                .map(IssuePriority::parse);
            let title = args.get("title").and_then(Value::as_str);
            let branch_name = args.get("branch_name").and_then(Value::as_str);
            let comment = args.get("comment").and_then(Value::as_str);
            let append_spec = args.get("append_spec").and_then(Value::as_str);
            let add_labels: Option<Vec<String>> = args
                .get("add_labels")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                });
            let author = args
                .get("author")
                .and_then(Value::as_str)
                .unwrap_or("agent");

            let existing = ctx
                .db
                .get_issue(issue_id)?
                .ok_or_else(|| anyhow::anyhow!("Issue '{issue_id}' not found"))?;

            let merged_spec = append_spec.map(|extra| {
                if existing.spec.trim().is_empty() {
                    extra.trim().to_string()
                } else {
                    format!("{}\n\n{}", existing.spec.trim_end(), extra.trim())
                }
            });

            let merged_labels = add_labels.map(|extra| {
                let mut labels = existing.labels.clone();
                for l in extra {
                    if !labels.contains(&l) {
                        labels.push(l);
                    }
                }
                labels
            });

            let updated = ctx.db.update_issue(
                &existing.id,
                title,
                merged_spec.as_deref(),
                status,
                priority,
                merged_labels,
                None,
                branch_name,
                comment.map(|c| (author, c)),
            )?;
            let _ = ctx.app.emit("factory-updated", ());
            Ok(serde_json::to_string_pretty(&updated)?)
        }
        "create_issue" => {
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing required argument: title"))?;
            let spec = args
                .get("spec")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let priority = args
                .get("priority")
                .and_then(Value::as_str)
                .map(IssuePriority::parse)
                .unwrap_or(IssuePriority::Medium);
            let labels = args
                .get("labels")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let workspace_path = args
                .get("workspace_path")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    ctx.db
                        .load_settings()
                        .recent_workspaces
                        .first()
                        .cloned()
                        .unwrap_or_else(|| ".".to_string())
                });

            let created = ctx
                .db
                .create_issue(&workspace_path, title, spec, priority, labels)?;
            let _ = ctx.app.emit("factory-updated", ());
            Ok(serde_json::to_string_pretty(&created)?)
        }
        unknown => Err(anyhow::anyhow!("Unknown MCP tool: {unknown}")),
    }
}

pub async fn build_factory_state(
    db: &Database,
    central_dir: &Path,
    mcp_port: u16,
) -> anyhow::Result<FactoryState> {
    let tabs = db.list_tabs()?;
    let reviews = db.list_reviews()?;
    let pending_reviews = reviews
        .iter()
        .filter(|r| r.status == ReviewStatus::Pending)
        .count();

    let mut seen_repos = HashSet::new();
    let mut dev_branch_heads = Vec::new();
    let mut active_worktrees = Vec::new();

    for tab in tabs {
        if tab.state == AgentState::Archived {
            continue;
        }
        let repo_key = (tab.repo_path.clone(), tab.base_branch.clone());
        if seen_repos.insert(repo_key) {
            let head = get_head_commit(Path::new(&tab.repo_path), &tab.base_branch).await;
            dev_branch_heads.push(RepoHeadInfo {
                repo_path: tab.repo_path.clone(),
                branch: tab.base_branch.clone(),
                head_commit: head,
            });
        }

        let touched = get_touched_files(Path::new(&tab.worktree_path), &tab.base_branch).await;
        active_worktrees.push(WorktreeStateInfo {
            tab_id: tab.id,
            feature_id: tab.feature_id,
            slug: tab.slug,
            branch_name: tab.branch_name,
            base_branch: tab.base_branch,
            worktree_path: tab.worktree_path,
            state: tab.state,
            touched_files: touched,
        });
    }

    Ok(FactoryState {
        mcp_port,
        central_dir: central_dir.to_string_lossy().to_string(),
        dev_branch_heads,
        active_worktrees,
        pending_reviews,
    })
}
