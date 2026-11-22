pub mod central;
pub mod changelog;
pub mod commands;
pub mod db;
pub mod git;
pub mod mcp;
pub mod models;
pub mod pty;

use central::{default_central_dir, ensure_central_config};
use commands::AppState;
use db::Database;
use mcp::start_mcp_server;
use pty::PtyManager;
use std::sync::{Arc, RwLock};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "grex=info,tower_http=info".into()),
        )
        .init();

    let central_dir = default_central_dir();
    let _ = std::fs::create_dir_all(&central_dir);

    let db_path = central_dir.join("grex-state.db");
    let db = Arc::new(
        Database::open(&db_path).expect("failed to initialize SQLite state database"),
    );

    let app_state = AppState {
        db: db.clone(),
        pty: PtyManager::new(),
        central_dir: central_dir.clone(),
        mcp_port: RwLock::new(4739),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let db_clone = db.clone();
            let central_clone = central_dir.clone();

            // Bootstrap ~/.agent-factory/ asynchronously so cold startup is instant
            let bootstrap_dir = central_dir.clone();
            let bootstrap_handle = handle.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(err) = ensure_central_config(&bootstrap_dir) {
                    tracing::warn!("Failed to bootstrap central ~/.agent-factory: {err}");
                } else {
                    let _ = bootstrap_handle.emit("factory-updated", ());
                }
            });

            tauri::async_runtime::spawn(async move {
                match start_mcp_server(handle.clone(), db_clone, central_clone).await {
                    Ok(port) => {
                        if let Some(st) = handle.try_state::<AppState>() {
                            *st.mcp_port.write().unwrap() = port;
                        }
                        tracing::info!("Grex embedded MCP server listening on 127.0.0.1:{port}");
                    }
                    Err(err) => {
                        tracing::error!("Failed to start Grex MCP server: {err}");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_state,
            commands::detect_git_branch,
            commands::create_agent_tab,
            commands::ensure_pty_running,
            commands::write_pty,
            commands::resize_pty,
            commands::close_tab,
            commands::request_changes,
            commands::approve_changelog,
            commands::dismiss_ping,
            commands::read_central_file,
            commands::save_central_file,
            commands::read_changelog_html,
            commands::get_factory_state_cmd,
            commands::submit_sample_changelog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Grex Tauri application");
}
