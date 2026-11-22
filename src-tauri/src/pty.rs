use crate::commands::AppState;
use crate::models::{AgentCli, AgentPermission, AgentState, PtyOutputPayload};
use anyhow::{anyhow, Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    last_activity: Arc<AtomicU64>,
}

#[derive(Clone, Default)]
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<String, PtySession>>>,
}

pub struct SpawnPtyOptions<'a> {
    pub tab_id: &'a str,
    pub feature_id: &'a str,
    pub worktree_path: &'a Path,
    pub central_dir: &'a Path,
    pub mcp_port: u16,
    pub mcp_config_path: Option<&'a Path>,
    pub cli: &'a AgentCli,
    pub permission: &'a AgentPermission,
    pub initial_prompt: &'a str,
    pub cols: u16,
    pub rows: u16,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn_session(&self, app: AppHandle, opts: SpawnPtyOptions<'_>) -> Result<()> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: opts.rows.max(10),
                cols: opts.cols.max(40),
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("open native PTY")?;

        let shell_program = default_shell();
        let mut cmd = CommandBuilder::new(&shell_program);
        cmd.cwd(opts.worktree_path);

        cmd.env("AGENT_FEATURE_ID", opts.feature_id);
        cmd.env("FACTORY_MCP_PORT", opts.mcp_port.to_string());
        cmd.env(
            "FACTORY_MCP_URL",
            format!("http://127.0.0.1:{}/mcp", opts.mcp_port),
        );
        cmd.env(
            "GREX_CENTRAL_DIR",
            opts.central_dir.to_string_lossy().to_string(),
        );
        cmd.env(
            "GREX_WORKTREE_PATH",
            opts.worktree_path.to_string_lossy().to_string(),
        );
        cmd.env("GREX_PERMISSION_MODE", opts.permission.as_str());
        cmd.env("TERM", "xterm-256color");

        let _child = pair
            .slave
            .spawn_command(cmd)
            .context("spawn shell inside PTY")?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("clone PTY reader")?;
        let mut writer = pair.master.take_writer().context("take PTY writer")?;

        if let Some(launch_cmd) = build_cli_launch_command(
            opts.cli,
            opts.permission,
            opts.mcp_config_path,
            opts.initial_prompt,
        ) {
            let line = format!("{launch_cmd}\r\n");
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.flush();
        }

        let tab_id_owned = opts.tab_id.to_string();
        let feature_id_owned = opts.feature_id.to_string();
        let last_activity = Arc::new(AtomicU64::new(now_secs()));

        {
            let mut guard = self.sessions.lock().unwrap();
            guard.insert(
                tab_id_owned.clone(),
                PtySession {
                    master: pair.master,
                    writer,
                    last_activity: last_activity.clone(),
                },
            );
        }

        // Background idle/inactivity monitor for this PTY
        let idle_app = app.clone();
        let idle_tab_id = tab_id_owned.clone();
        let idle_feature = feature_id_owned.clone();
        let idle_last = last_activity.clone();
        let idle_sessions = self.sessions.clone();
        tauri::async_runtime::spawn(async move {
            let mut marked_inactive = false;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if !idle_sessions.lock().unwrap().contains_key(&idle_tab_id) {
                    break;
                }
                let Some(st) = idle_app.try_state::<AppState>() else {
                    break;
                };
                let settings = st.db.load_settings();
                let timeout = settings.inactive_timeout_secs.max(10);
                let elapsed = now_secs().saturating_sub(idle_last.load(Ordering::Relaxed));

                if elapsed >= timeout {
                    if !marked_inactive {
                        if let Ok(Some(tab)) = st.db.get_tab_by_id(&idle_tab_id) {
                            if tab.state == AgentState::Working {
                                let _ = st.db.update_tab_state(&idle_tab_id, AgentState::Inactive);
                                let _ = idle_app.emit("factory-updated", ());
                                if settings.notify_on_inactive {
                                    let _ = idle_app
                                        .notification()
                                        .builder()
                                        .title(&format!("Grex Agent Inactive: {idle_feature}"))
                                        .body("Agent has stopped producing output and may be waiting for input.")
                                        .show();
                                }
                                marked_inactive = true;
                            }
                        }
                    }
                } else {
                    marked_inactive = false;
                }
            }
        });

        // Stream PTY output asynchronously to xterm.js via Tauri events
        let sessions_ref = self.sessions.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        last_activity.store(now_secs(), Ordering::Relaxed);
                        if let Some(st) = app.try_state::<AppState>() {
                            if let Ok(Some(tab)) = st.db.get_tab_by_id(&tab_id_owned) {
                                if tab.state == AgentState::Inactive {
                                    let _ = st.db.update_tab_state(&tab_id_owned, AgentState::Working);
                                    let _ = app.emit("factory-updated", ());
                                }
                            }
                        }
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = app.emit(
                            "pty-output",
                            PtyOutputPayload {
                                tab_id: tab_id_owned.clone(),
                                data: text,
                            },
                        );
                    }
                    Err(_) => break,
                }
            }
            if let Some(st) = app.try_state::<AppState>() {
                if let Ok(Some(tab)) = st.db.get_tab_by_id(&tab_id_owned) {
                    if tab.state == AgentState::Working {
                        let _ = st.db.update_tab_state(&tab_id_owned, AgentState::Inactive);
                        let _ = app.emit("factory-updated", ());
                        let settings = st.db.load_settings();
                        if settings.notify_on_inactive {
                            let _ = app
                                .notification()
                                .builder()
                                .title(&format!("Grex Agent Inactive: {feature_id_owned}"))
                                .body("Agent shell exited.")
                                .show();
                        }
                    }
                }
            }
            let _ = app.emit("pty-closed", tab_id_owned.clone());
            sessions_ref.lock().unwrap().remove(&tab_id_owned);
        });

        Ok(())
    }

    pub fn write(&self, tab_id: &str, data: &str) -> Result<()> {
        let mut guard = self.sessions.lock().unwrap();
        let session = guard
            .get_mut(tab_id)
            .ok_or_else(|| anyhow!("No active PTY session for tab {tab_id}"))?;
        session.last_activity.store(now_secs(), Ordering::Relaxed);
        session.writer.write_all(data.as_bytes())?;
        session.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, tab_id: &str, cols: u16, rows: u16) -> Result<()> {
        let guard = self.sessions.lock().unwrap();
        if let Some(session) = guard.get(tab_id) {
            session.master.resize(PtySize {
                rows: rows.max(4),
                cols: cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        Ok(())
    }

    pub fn close(&self, tab_id: &str) {
        let mut guard = self.sessions.lock().unwrap();
        if let Some(mut session) = guard.remove(tab_id) {
            let _ = session.writer.write_all(b"exit\r\n");
            let _ = session.writer.flush();
        }
    }

    pub fn has_session(&self, tab_id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(tab_id)
    }
}

fn default_shell() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "powershell.exe".to_string())
    } else {
        std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/bash".to_string())
    }
}

fn build_cli_launch_command(
    cli: &AgentCli,
    permission: &AgentPermission,
    mcp_config_path: Option<&Path>,
    initial_prompt: &str,
) -> Option<String> {
    let prompt_trimmed = initial_prompt.trim();
    let escaped_prompt = prompt_trimmed.replace('"', "\\\"");
    let mcp_flag = mcp_config_path
        .map(|p| format!("--mcp-config \"{}\"", p.to_string_lossy()))
        .unwrap_or_default();

    match cli {
        AgentCli::Shell => None,
        AgentCli::Claude => {
            let perm_flag = match permission {
                AgentPermission::Default => "",
                AgentPermission::Auto => "--permission-mode acceptEdits",
                AgentPermission::DangerouslySkip => "--dangerously-skip-permissions",
            };
            let parts = ["claude", perm_flag, &mcp_flag]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if prompt_trimmed.is_empty() {
                Some(parts)
            } else {
                Some(format!("{parts} \"{escaped_prompt}\""))
            }
        }
        AgentCli::Codex => {
            let perm_flag = match permission {
                AgentPermission::Default => "",
                AgentPermission::Auto => "--full-auto",
                AgentPermission::DangerouslySkip => "--dangerously-bypass-approvals-and-sandbox",
            };
            let parts = ["codex", perm_flag]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if prompt_trimmed.is_empty() {
                Some(parts)
            } else {
                Some(format!("{parts} \"{escaped_prompt}\""))
            }
        }
        AgentCli::Antigravity => {
            let perm_flag = match permission {
                AgentPermission::Default => "",
                AgentPermission::Auto => "--auto",
                AgentPermission::DangerouslySkip => "--yolo",
            };
            let parts = ["agy", perm_flag]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if prompt_trimmed.is_empty() {
                Some(parts)
            } else {
                Some(format!("{parts} \"{escaped_prompt}\""))
            }
        }
    }
}
