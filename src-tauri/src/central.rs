use anyhow::{anyhow, Context, Result};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const DEFAULT_AGENTS_MD: &str = include_str!("../../templates/agent-factory/AGENTS.md");
const DEFAULT_WORKTREE_SKILL: &str =
    include_str!("../../templates/agent-factory/.skills/worktree-workflow/SKILL.md");
const DEFAULT_SELF_IMPROVE_SKILL: &str =
    include_str!("../../templates/agent-factory/.skills/factory-self-improve/SKILL.md");
pub const SAMPLE_CHANGELOG_HTML: &str = include_str!("../../templates/sample-changelog.html");

pub fn default_central_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".agent-factory")
}

/// Ensures `~/.agent-factory/` exists, contains default `AGENTS.md` and meta-skills,
/// and is initialized as a git repository so central instruction updates have history.
pub fn ensure_central_config(central_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(central_dir.join(".skills/worktree-workflow"))
        .context("create worktree-workflow skill dir")?;
    std::fs::create_dir_all(central_dir.join(".skills/factory-self-improve"))
        .context("create factory-self-improve skill dir")?;

    let mut created_any = false;

    let files = [
        ("AGENTS.md", DEFAULT_AGENTS_MD),
        (".skills/worktree-workflow/SKILL.md", DEFAULT_WORKTREE_SKILL),
        (
            ".skills/factory-self-improve/SKILL.md",
            DEFAULT_SELF_IMPROVE_SKILL,
        ),
    ];

    for (rel, content) in files {
        let full = central_dir.join(rel);
        if !full.exists() {
            atomic_write(&full, content)?;
            created_any = true;
        }
    }

    if !central_dir.join(".git").exists() {
        let _ = Command::new("git")
            .args(["init", "-q"])
            .current_dir(central_dir)
            .output();
        let _ = Command::new("git")
            .args(["config", "user.name", "Grex Factory"])
            .current_dir(central_dir)
            .output();
        let _ = Command::new("git")
            .args(["config", "user.email", "grex@local.factory"])
            .current_dir(central_dir)
            .output();
        created_any = true;
    }

    if created_any {
        let _ = git_commit_all(central_dir, "Initialize Grex central AGENTS.md and meta-skills");
    }

    Ok(())
}

/// Validates that `target_file` stays strictly inside `central_dir` (no path traversal).
pub fn resolve_safe_central_path(central_dir: &Path, target_file: &str) -> Result<PathBuf> {
    let rel = Path::new(target_file);
    if rel.is_absolute() {
        return Err(anyhow!(
            "target_file must be relative to central directory: {target_file}"
        ));
    }
    for comp in rel.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!(
                    "Invalid path traversal in target_file: {target_file}"
                ));
            }
            _ => {}
        }
    }
    Ok(central_dir.join(rel))
}

/// Atomically writes `content` to `dest` via a sibling temp file + rename.
pub fn atomic_write(dest: &Path, content: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_name = format!(
        ".{}.{}.tmp",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        uuid::Uuid::new_v4()
    );
    let tmp_path = dest.with_file_name(tmp_name);
    std::fs::write(&tmp_path, content)?;
    if let Err(err) = std::fs::rename(&tmp_path, dest) {
        // On Windows rename over an existing file can occasionally fail; fallback to copy + remove.
        std::fs::copy(&tmp_path, dest).with_context(|| format!("atomic write fallback: {err}"))?;
        let _ = std::fs::remove_file(&tmp_path);
    }
    Ok(())
}

/// Safely updates a file inside `~/.agent-factory/` with atomic write + git commit.
pub fn update_central_file(
    central_dir: &Path,
    target_file: &str,
    content: &str,
    commit_message: Option<&str>,
) -> Result<String> {
    ensure_central_config(central_dir)?;
    let full = resolve_safe_central_path(central_dir, target_file)?;
    atomic_write(&full, content)?;

    let msg = commit_message
        .filter(|m| !m.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Update central instruction: {target_file}"));

    let commit_hash = git_commit_all(central_dir, &msg).unwrap_or_else(|_| "uncommitted".into());
    Ok(commit_hash)
}

pub fn list_central_files(central_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_files(central_dir, central_dir, &mut out);
    out.sort();
    out
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn git_commit_all(dir: &Path, message: &str) -> Result<String> {
    let _ = Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output()?;
    let _ = Command::new("git")
        .args(["commit", "-m", message, "--allow-empty"])
        .current_dir(dir)
        .output()?;
    let rev = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(dir)
        .output()?;
    Ok(String::from_utf8_lossy(&rev.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn bootstrap_and_atomic_update_work() {
        let tmp = TempDir::new().unwrap();
        ensure_central_config(tmp.path()).unwrap();
        assert!(tmp.path().join("AGENTS.md").exists());
        assert!(tmp
            .path()
            .join(".skills/worktree-workflow/SKILL.md")
            .exists());
        assert!(tmp
            .path()
            .join(".skills/factory-self-improve/SKILL.md")
            .exists());

        let hash = update_central_file(
            tmp.path(),
            ".skills/custom/SKILL.md",
            "# Custom Skill\n",
            Some("Add custom skill"),
        )
        .unwrap();
        assert!(!hash.is_empty());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(".skills/custom/SKILL.md")).unwrap(),
            "# Custom Skill\n"
        );
    }

    #[test]
    fn blocks_path_traversal() {
        let tmp = TempDir::new().unwrap();
        assert!(resolve_safe_central_path(tmp.path(), "../evil.txt").is_err());
        assert!(resolve_safe_central_path(tmp.path(), "/etc/passwd").is_err());
    }
}
