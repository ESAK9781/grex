use crate::models::{AgentCli, PtyOutputPayload};
use anyhow::{anyhow, Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
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
    pub initial_prompt: &'a str,
    pub cols: u16,
    pub rows: u16,
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

        // Inject Grex factory environment variables
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

        // Auto-launch CLI startup command + initial prompt if configured
        if let Some(launch_cmd) =
            build_cli_launch_command(opts.cli, opts.mcp_config_path, opts.initial_prompt)
        {
            let line = format!("{launch_cmd}\r\n");
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.flush();
        }

        let tab_id_owned = opts.tab_id.to_string();
        {
            let mut guard = self.sessions.lock().unwrap();
            guard.insert(
                tab_id_owned.clone(),
                PtySession {
                    master: pair.master,
                    writer,
                },
            );
        }

        // Stream PTY output asynchronously to xterm.js via Tauri events
        let sessions_ref = self.sessions.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
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
            if prompt_trimmed.is_empty() {
                Some(format!("claude {mcp_flag}").trim().to_string())
            } else {
                Some(format!("claude {mcp_flag} \"{escaped_prompt}\""))
            }
        }
        AgentCli::Codex => {
            if prompt_trimmed.is_empty() {
                Some("codex".to_string())
            } else {
                Some(format!("codex \"{escaped_prompt}\""))
            }
        }
        AgentCli::Antigravity => {
            if prompt_trimmed.is_empty() {
                Some("agy".to_string())
            } else {
                Some(format!("agy \"{escaped_prompt}\""))
            }
        }
    }
}
