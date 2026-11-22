use crate::central::update_central_file;
use crate::changelog::validate_changelog_file;
use crate::db::Database;
use crate::git::{get_head_commit, get_touched_files};
use crate::models::{
    AgentState, FactoryState, PingEventPayload, PingUrgency, RepoHeadInfo, ReviewStatus,
    WorktreeStateInfo,
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
