use crate::central::atomic_write;
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;

fn hidden_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledMcp {
    pub id: String,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub installed_path: Option<String>,
    pub installed_at: String,
}

fn default_transport() -> String {
    "stdio".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpRegistryFile {
    pub servers: Vec<InstalledMcp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCustomMcpRequest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

pub fn registry_path(central_dir: &Path) -> PathBuf {
    central_dir.join("mcps.json")
}

pub fn load_registry(central_dir: &Path) -> Vec<InstalledMcp> {
    let path = registry_path(central_dir);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<McpRegistryFile>(&raw)
        .map(|f| f.servers)
        .unwrap_or_default()
}

pub fn save_registry(central_dir: &Path, servers: &[InstalledMcp]) -> Result<()> {
    let file = McpRegistryFile {
        servers: servers.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    atomic_write(&registry_path(central_dir), &json)?;
    Ok(())
}

pub fn upsert_mcp(central_dir: &Path, entry: InstalledMcp) -> Result<Vec<InstalledMcp>> {
    let mut servers = load_registry(central_dir);
    if let Some(existing) = servers.iter_mut().find(|s| s.id == entry.id) {
        *existing = entry;
    } else {
        servers.push(entry);
    }
    save_registry(central_dir, &servers)?;
    Ok(servers)
}

pub fn toggle_mcp(central_dir: &Path, id: &str, enabled: bool) -> Result<Vec<InstalledMcp>> {
    let mut servers = load_registry(central_dir);
    let target = servers
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| anyhow!("MCP server '{id}' not found"))?;
    target.enabled = enabled;
    save_registry(central_dir, &servers)?;
    Ok(servers)
}

pub fn update_mcp_env(
    central_dir: &Path,
    id: &str,
    env: BTreeMap<String, String>,
) -> Result<Vec<InstalledMcp>> {
    let mut servers = load_registry(central_dir);
    let target = servers
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| anyhow!("MCP server '{id}' not found"))?;
    target.env = env;
    save_registry(central_dir, &servers)?;
    Ok(servers)
}

pub fn remove_mcp(central_dir: &Path, id: &str) -> Result<Vec<InstalledMcp>> {
    let mut servers = load_registry(central_dir);
    if let Some(pos) = servers.iter().position(|s| s.id == id) {
        let removed = servers.remove(pos);
        if let Some(installed_path) = removed.installed_path {
            let p = PathBuf::from(installed_path);
            let mcps_root = central_dir.join("mcps");
            if p.starts_with(&mcps_root) && p.exists() {
                let _ = std::fs::remove_dir_all(&p);
            }
        }
    }
    save_registry(central_dir, &servers)?;
    Ok(servers)
}

/// Parses a GitHub repo reference (`owner/repo` or `https://github.com/owner/repo[.git]`).
pub fn parse_github_repo(input: &str) -> Result<(String, String, String)> {
    let trimmed = input.trim().trim_end_matches('/').trim_end_matches(".git");
    let stripped = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))
        .unwrap_or(trimmed);

    let parts: Vec<&str> = stripped.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        return Err(anyhow!(
            "Expected a GitHub repo like 'owner/repo' or 'https://github.com/owner/repo'"
        ));
    }
    let owner = parts[0].to_string();
    let repo = parts[1].to_string();
    let clone_url = format!("https://github.com/{owner}/{repo}.git");
    Ok((owner, repo, clone_url))
}

/// Clones a GitHub MCP repository into `~/.agent-factory/mcps/<slug>`,
/// auto-detects Node/TypeScript, Python, or Rust, builds if needed, and registers it.
pub async fn install_from_github(
    central_dir: &Path,
    repo_input: &str,
    custom_id: Option<&str>,
    env: BTreeMap<String, String>,
) -> Result<InstalledMcp> {
    let (owner, repo, clone_url) = parse_github_repo(repo_input)?;
    let slug = sanitize_id(custom_id.unwrap_or(&repo));
    if slug.is_empty() {
        return Err(anyhow!("Could not derive a valid MCP id from '{repo_input}'"));
    }

    let mcps_dir = central_dir.join("mcps");
    tokio::fs::create_dir_all(&mcps_dir).await?;
    let target_dir = mcps_dir.join(&slug);

    if target_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&target_dir).await;
    }

    let clone_out = hidden_cmd("git")
        .args([
            "clone",
            "--depth",
            "1",
            &clone_url,
            &target_dir.to_string_lossy(),
        ])
        .output()
        .await
        .context("run git clone")?;

    if !clone_out.status.success() {
        let stderr = String::from_utf8_lossy(&clone_out.stderr);
        return Err(anyhow!("git clone failed: {stderr}"));
    }

    let (command, args) = detect_and_prepare_runtime(&target_dir).await?;

    let entry = InstalledMcp {
        id: slug.clone(),
        name: repo.clone(),
        source: format!("github:{owner}/{repo}"),
        repo_url: Some(format!("https://github.com/{owner}/{repo}")),
        transport: "stdio".to_string(),
        command,
        args,
        url: None,
        env,
        enabled: true,
        installed_path: Some(target_dir.to_string_lossy().to_string()),
        installed_at: Utc::now().to_rfc3339(),
    };

    upsert_mcp(central_dir, entry.clone())?;
    Ok(entry)
}

async fn detect_and_prepare_runtime(dir: &Path) -> Result<(String, Vec<String>)> {
    let pkg_json_path = dir.join("package.json");
    if pkg_json_path.exists() {
        let raw = tokio::fs::read_to_string(&pkg_json_path).await?;
        let pkg: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);

        let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };
        let install_out = hidden_cmd(npm_cmd)
            .args(["install", "--no-audit", "--no-fund"])
            .current_dir(dir)
            .output()
            .await;
        if let Ok(out) = install_out {
            if !out.status.success() {
                tracing::warn!(
                    "npm install warning in {}: {}",
                    dir.display(),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }

        if pkg
            .get("scripts")
            .and_then(|s| s.get("build"))
            .and_then(Value::as_str)
            .is_some()
        {
            let _ = hidden_cmd(npm_cmd)
                .args(["run", "build"])
                .current_dir(dir)
                .output()
                .await;
        }

        if let Some(entry) = resolve_node_entrypoint(dir, &pkg) {
            return Ok(("node".to_string(), vec![entry.to_string_lossy().to_string()]));
        }

        if let Some(pkg_name) = pkg.get("name").and_then(Value::as_str) {
            let npx = if cfg!(windows) { "npx.cmd" } else { "npx" };
            return Ok((npx.to_string(), vec!["-y".to_string(), pkg_name.to_string()]));
        }
    }

    if dir.join("pyproject.toml").exists() {
        return Ok((
            "uv".to_string(),
            vec![
                "--directory".to_string(),
                dir.to_string_lossy().to_string(),
                "run".to_string(),
                "python".to_string(),
                "-m".to_string(),
                guess_python_module(dir),
            ],
        ));
    }

    if dir.join("Cargo.toml").exists() {
        let build = hidden_cmd("cargo")
            .args(["build", "--release"])
            .current_dir(dir)
            .output()
            .await
            .context("cargo build --release for MCP")?;
        if build.status.success() {
            let release_dir = dir.join("target/release");
            if let Ok(entries) = std::fs::read_dir(&release_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        let is_exe = if cfg!(windows) {
                            p.extension().and_then(|e| e.to_str()) == Some("exe")
                        } else {
                            p.extension().is_none()
                        };
                        if is_exe {
                            return Ok((p.to_string_lossy().to_string(), Vec::new()));
                        }
                    }
                }
            }
        }
    }

    for py_entry in ["server.py", "main.py", "src/server.py"] {
        let p = dir.join(py_entry);
        if p.exists() {
            return Ok((
                "python".to_string(),
                vec![p.to_string_lossy().to_string()],
            ));
        }
    }

    Err(anyhow!(
        "Cloned repository to {}, but could not auto-detect Node, Python, or Rust MCP entrypoint. You can add its exact command via 'Import JSON / Custom'.",
        dir.display()
    ))
}

fn resolve_node_entrypoint(dir: &Path, pkg: &Value) -> Option<PathBuf> {
    if let Some(bin) = pkg.get("bin") {
        if let Some(s) = bin.as_str() {
            let p = dir.join(s);
            if p.exists() {
                return Some(p);
            }
        } else if let Some(obj) = bin.as_object() {
            for val in obj.values() {
                if let Some(s) = val.as_str() {
                    let p = dir.join(s);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }

    if let Some(main) = pkg.get("main").and_then(Value::as_str) {
        let p = dir.join(main);
        if p.exists() {
            return Some(p);
        }
    }

    for candidate in [
        "dist/index.js",
        "build/index.js",
        "dist/server.js",
        "build/server.js",
        "index.js",
        "server.js",
    ] {
        let p = dir.join(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn guess_python_module(dir: &Path) -> String {
    for candidate in ["server", "mcp_server", "main"] {
        if dir.join(format!("{candidate}.py")).exists()
            || dir.join("src").join(candidate).exists()
        {
            return candidate.to_string();
        }
    }
    "server".to_string()
}

/// Imports a standard `{ "mcpServers": { "name": { "command": ..., "args": [...], "env": {...} } } }` JSON snippet.
pub fn import_mcp_json(central_dir: &Path, raw_json: &str) -> Result<Vec<InstalledMcp>> {
    let parsed: Value = serde_json::from_str(raw_json)
        .context("Invalid JSON — expected a standard mcpServers JSON object")?;

    let servers_obj = parsed
        .get("mcpServers")
        .and_then(Value::as_object)
        .or_else(|| parsed.as_object())
        .ok_or_else(|| anyhow!("JSON must be an object or contain a 'mcpServers' object"))?;

    let mut imported = Vec::new();
    for (key, cfg) in servers_obj {
        let Some(obj) = cfg.as_object() else {
            continue;
        };
        let id = sanitize_id(key);
        if id.is_empty() || id == "grex" {
            continue;
        }
        let command = obj
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let args = obj
            .get("args")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let url = obj.get("url").and_then(Value::as_str).map(str::to_string);
        let transport = if url.is_some() && command.is_empty() {
            "http".to_string()
        } else {
            "stdio".to_string()
        };
        let mut env = BTreeMap::new();
        if let Some(env_obj) = obj.get("env").and_then(Value::as_object) {
            for (ek, ev) in env_obj {
                if let Some(s) = ev.as_str() {
                    env.insert(ek.clone(), s.to_string());
                }
            }
        }

        let entry = InstalledMcp {
            id: id.clone(),
            name: key.clone(),
            source: "json-import".to_string(),
            repo_url: None,
            transport,
            command,
            args,
            url,
            env,
            enabled: true,
            installed_path: None,
            installed_at: Utc::now().to_rfc3339(),
        };
        upsert_mcp(central_dir, entry.clone())?;
        imported.push(entry);
    }

    if imported.is_empty() {
        return Err(anyhow!("No valid MCP server definitions found in JSON"));
    }

    Ok(imported)
}

pub fn sanitize_id(raw: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parses_github_urls_and_shorthand() {
        let (o, r, url) = parse_github_repo("https://github.com/microsoft/playwright-mcp").unwrap();
        assert_eq!(o, "microsoft");
        assert_eq!(r, "playwright-mcp");
        assert_eq!(url, "https://github.com/microsoft/playwright-mcp.git");

        let (o2, r2, _) = parse_github_repo("upstash/context7").unwrap();
        assert_eq!(o2, "upstash");
        assert_eq!(r2, "context7");
    }

    #[test]
    fn imports_standard_mcp_servers_json_and_toggles() {
        let tmp = TempDir::new().unwrap();
        let json = r#"{
            "mcpServers": {
                "playwright": {
                    "command": "npx",
                    "args": ["-y", "@playwright/mcp@latest"]
                },
                "github": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"],
                    "env": { "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_test" }
                }
            }
        }"#;
        let imported = import_mcp_json(tmp.path(), json).unwrap();
        assert_eq!(imported.len(), 2);

        let all = load_registry(tmp.path());
        assert_eq!(all.len(), 2);

        let toggled = toggle_mcp(tmp.path(), "playwright", false).unwrap();
        assert!(!toggled.iter().find(|m| m.id == "playwright").unwrap().enabled);

        let remaining = remove_mcp(tmp.path(), "playwright").unwrap();
        assert_eq!(remaining.len(), 1);
    }
}
