use anyhow::{anyhow, Context, Result};
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

pub async fn detect_default_branch(repo_path: &Path) -> String {
    if let Ok(out) = hidden_cmd("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .await
    {
        if out.status.success() {
            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !b.is_empty() && b != "HEAD" {
                return b;
            }
        }
    }
    "main".to_string()
}

pub async fn get_head_commit(repo_path: &Path, branch: &str) -> String {
    if let Ok(out) = hidden_cmd("git")
        .args(["rev-parse", "--short", branch])
        .current_dir(repo_path)
        .output()
        .await
    {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
    }
    "unknown".to_string()
}

/// Runs `git worktree add .worktrees/<slug> -b agent/<slug> <base_branch>`.
pub async fn create_worktree(
    repo_path: &Path,
    slug: &str,
    base_branch: &str,
) -> Result<(PathBuf, String)> {
    let worktrees_root = repo_path.join(".worktrees");
    tokio::fs::create_dir_all(&worktrees_root)
        .await
        .context("create .worktrees directory")?;

    let worktree_path = worktrees_root.join(slug);
    let branch_name = format!("agent/{slug}");

    let out = hidden_cmd("git")
        .args([
            "worktree",
            "add",
            &worktree_path.to_string_lossy(),
            "-b",
            &branch_name,
            base_branch,
        ])
        .current_dir(repo_path)
        .output()
        .await
        .context("spawn git worktree add")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("git worktree add failed: {stderr}"));
    }

    Ok((worktree_path, branch_name))
}

/// Symlinks `~/.agent-factory/AGENTS.md` and `.skills/` into the worktree
/// (gracefully falling back to copying if OS symlink privileges are restricted)
/// and writes `.grex-mcp.json` pointing to the embedded Grex MCP server.
pub fn inject_central_config(
    worktree_path: &Path,
    central_dir: &Path,
    mcp_port: u16,
    feature_id: &str,
) -> Result<PathBuf> {
    let central_agents = central_dir.join("AGENTS.md");
    let target_agents = worktree_path.join("AGENTS.md");
    if central_agents.exists() && !target_agents.exists() {
        link_or_copy_file(&central_agents, &target_agents)?;
    }

    let central_skills = central_dir.join(".skills");
    let target_skills = worktree_path.join(".skills");
    if central_skills.exists() && !target_skills.exists() {
        link_or_copy_dir(&central_skills, &target_skills)?;
    }

    // Write MCP config for agent CLIs (--mcp-config), merging Grex's embedded MCP
    // with all enabled GitHub/custom MCPs from ~/.agent-factory/mcps.json
    let shim_bin = resolve_mcp_shim_binary();
    let mut servers_map = serde_json::Map::new();
    servers_map.insert(
        "grex".to_string(),
        serde_json::json!({
            "command": shim_bin,
            "args": [],
            "env": {
                "FACTORY_MCP_PORT": mcp_port.to_string(),
                "FACTORY_MCP_URL": format!("http://127.0.0.1:{mcp_port}/mcp"),
                "AGENT_FEATURE_ID": feature_id
            },
            "url": format!("http://127.0.0.1:{mcp_port}/sse")
        }),
    );

    for mcp in crate::mcps::load_registry(central_dir) {
        if !mcp.enabled || mcp.id == "grex" {
            continue;
        }
        let mut entry = serde_json::Map::new();
        if !mcp.command.is_empty() {
            entry.insert("command".to_string(), serde_json::json!(mcp.command));
            entry.insert("args".to_string(), serde_json::json!(mcp.args));
        }
        if let Some(url) = mcp.url {
            entry.insert("url".to_string(), serde_json::json!(url));
        }
        if !mcp.env.is_empty() {
            entry.insert("env".to_string(), serde_json::json!(mcp.env));
        }
        servers_map.insert(mcp.id, serde_json::Value::Object(entry));
    }

    let mcp_config = serde_json::json!({
        "mcpServers": serde_json::Value::Object(servers_map)
    });

    let mcp_config_path = worktree_path.join(".grex-mcp.json");
    std::fs::write(&mcp_config_path, serde_json::to_string_pretty(&mcp_config)?)?;

    Ok(mcp_config_path)
}

fn resolve_mcp_shim_binary() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = if cfg!(windows) {
                dir.join("grex-mcp.exe")
            } else {
                dir.join("grex-mcp")
            };
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    "grex-mcp".to_string()
}

fn link_or_copy_file(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(unix)]
    let res = std::os::unix::fs::symlink(src, dst);
    #[cfg(windows)]
    let res = std::os::windows::fs::symlink_file(src, dst);

    if res.is_err() {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn link_or_copy_dir(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(unix)]
    let res = std::os::unix::fs::symlink(src, dst);
    #[cfg(windows)]
    let res = std::os::windows::fs::symlink_dir(src, dst);

    if res.is_err() {
        copy_dir_recursive(src, dst)?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

/// Returns both committed diff files vs `base_branch` and uncommitted dirty files in `worktree_path`.
pub async fn get_touched_files(worktree_path: &Path, base_branch: &str) -> Vec<String> {
    let mut files = std::collections::BTreeSet::new();

    if let Ok(out) = hidden_cmd("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()
        .await
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if line.len() > 3 {
                    let f = line[3..].trim();
                    if !f.is_empty() && f != ".grex-mcp.json" {
                        files.insert(f.to_string());
                    }
                }
            }
        }
    }

    let range = format!("{base_branch}..HEAD");
    if let Ok(out) = hidden_cmd("git")
        .args(["diff", "--name-only", &range])
        .current_dir(worktree_path)
        .output()
        .await
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let f = line.trim();
                if !f.is_empty() {
                    files.insert(f.to_string());
                }
            }
        }
    }

    files.into_iter().collect()
}

/// Merges `branch_name` into `base_branch` with `--no-ff`, removes the worktree, and deletes the merged branch.
pub async fn approve_and_merge(
    repo_path: &Path,
    worktree_path: &Path,
    branch_name: &str,
    base_branch: &str,
) -> Result<String> {
    // Ensure main repo is on base_branch
    let checkout = hidden_cmd("git")
        .args(["checkout", base_branch])
        .current_dir(repo_path)
        .output()
        .await
        .context("git checkout base_branch")?;
    if !checkout.status.success() {
        return Err(anyhow!(
            "git checkout {base_branch} failed: {}",
            String::from_utf8_lossy(&checkout.stderr)
        ));
    }

    let merge = hidden_cmd("git")
        .args([
            "merge",
            "--no-ff",
            branch_name,
            "-m",
            &format!("Merge {branch_name} into {base_branch} (Grex Approved)"),
        ])
        .current_dir(repo_path)
        .output()
        .await
        .context("git merge --no-ff")?;

    if !merge.status.success() {
        return Err(anyhow!(
            "git merge --no-ff {branch_name} failed: {}",
            String::from_utf8_lossy(&merge.stderr)
        ));
    }

    let _ = remove_worktree(repo_path, worktree_path, branch_name).await;
    Ok(String::from_utf8_lossy(&merge.stdout).trim().to_string())
}

pub async fn remove_worktree(
    repo_path: &Path,
    worktree_path: &Path,
    branch_name: &str,
) -> Result<()> {
    let _ = hidden_cmd("git")
        .args([
            "worktree",
            "remove",
            "--force",
            &worktree_path.to_string_lossy(),
        ])
        .current_dir(repo_path)
        .output()
        .await;

    let _ = hidden_cmd("git")
        .args(["branch", "-d", branch_name])
        .current_dir(repo_path)
        .output()
        .await;

    Ok(())
}
