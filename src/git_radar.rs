use std::fs;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::config_patcher::home_dir;
use crate::platform;

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub changed_count: usize,
    pub ahead: usize,
    pub behind: usize,
    pub last_commit: String,
}

impl RepoStatus {
    pub fn is_dirty(&self) -> bool {
        self.changed_count > 0
    }
}

pub fn expand_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        home_dir().join(rest)
    } else if trimmed == "~" {
        home_dir()
    } else {
        PathBuf::from(trimmed)
    }
}

pub async fn scan_projects(dir_raw: String) -> Result<Vec<RepoStatus>, String> {
    let Some(git_bin) = platform::find_in_path("git") else {
        return Err("Утилита git не найдена в PATH.".to_string());
    };

    let root = expand_path(&dir_raw);
    if !root.exists() {
        return Err(format!(
            "Каталог проектов «{}» не найден (настройте projects_dir в config.toml).",
            root.display()
        ));
    }
    if !root.is_dir() {
        return Err(format!("Путь «{}» не является каталогом.", root.display()));
    }

    let candidate_dirs = tokio::task::spawn_blocking({
        let root = root.clone();
        move || -> Result<Vec<PathBuf>, String> {
            let mut dirs = Vec::new();
            if root.join(".git").exists() {
                dirs.push(root.clone());
            }
            let entries = fs::read_dir(&root)
                .map_err(|e| format!("Не удалось прочитать {}: {e}", root.display()))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
                if path.join(".git").exists() {
                    dirs.push(path);
                }
            }
            Ok(dirs)
        }
    })
    .await
    .map_err(|e| format!("Ошибка сканирования каталога: {e}"))??;

    let mut repos = Vec::with_capacity(candidate_dirs.len());
    for dir in candidate_dirs {
        if let Some(st) = inspect_repo(&git_bin, &dir).await {
            repos.push(st);
        }
    }

    repos.sort_by(|a, b| {
        b.changed_count
            .cmp(&a.changed_count)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(repos)
}

async fn inspect_repo(git_bin: &Path, path: &Path) -> Option<RepoStatus> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let out = Command::new(git_bin)
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain=v1", "-b"])
        .output()
        .await
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    let header = lines.next().unwrap_or("");

    let (branch, ahead, behind) = parse_branch_header(header);
    let changed_count = lines.filter(|l| !l.trim().is_empty()).count();

    let last_commit = Command::new(git_bin)
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-C")
        .arg(path)
        .args(["log", "-1", "--pretty=format:%s"])
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    Some(RepoStatus {
        name,
        path: path.to_path_buf(),
        branch,
        changed_count,
        ahead,
        behind,
        last_commit,
    })
}

pub fn parse_branch_header(header: &str) -> (String, usize, usize) {
    let body = header.strip_prefix("## ").unwrap_or(header).trim();
    if body.is_empty() {
        return ("HEAD".to_string(), 0, 0);
    }
    if let Some(rest) = body.strip_prefix("No commits yet on ") {
        return (rest.trim().to_string(), 0, 0);
    }

    let mut ahead = 0usize;
    let mut behind = 0usize;

    let branch_part = if let Some((left, bracket)) = body.split_once(" [") {
        let inner = bracket.trim_end_matches(']');
        for token in inner.split(',') {
            let t = token.trim();
            if let Some(n) = t.strip_prefix("ahead ") {
                ahead = n.trim().parse().unwrap_or(0);
            } else if let Some(n) = t.strip_prefix("behind ") {
                behind = n.trim().parse().unwrap_or(0);
            }
        }
        left
    } else {
        body
    };

    let branch = branch_part
        .split("...")
        .next()
        .unwrap_or(branch_part)
        .trim()
        .to_string();

    (branch, ahead, behind)
}

pub async fn quick_commit_and_push(
    repo: RepoStatus,
    custom_msg: Option<String>,
) -> Result<String, String> {
    let git_bin = platform::find_in_path("git")
        .ok_or_else(|| "Утилита git не найдена в PATH.".to_string())?;

    let msg = custom_msg
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("chore: quick commit via zipctl")
        .to_string();

    let add_out = Command::new(&git_bin)
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-C")
        .arg(&repo.path)
        .args(["add", "-A"])
        .output()
        .await
        .map_err(|e| format!("Ошибка запуска git add: {e}"))?;

    if !add_out.status.success() {
        let err = String::from_utf8_lossy(&add_out.stderr);
        return Err(format!("git add ({}) ошибка: {}", repo.name, err.trim()));
    }

    let mut committed = false;
    let commit_out = Command::new(&git_bin)
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-C")
        .arg(&repo.path)
        .args(["commit", "-m", &msg])
        .output()
        .await
        .map_err(|e| format!("Ошибка запуска git commit: {e}"))?;

    if commit_out.status.success() {
        committed = true;
    } else {
        let combined = format!(
            "{} {}",
            String::from_utf8_lossy(&commit_out.stdout),
            String::from_utf8_lossy(&commit_out.stderr)
        );
        if !combined.contains("nothing to commit") && !combined.contains("нечего коммитить") {
            return Err(format!(
                "git commit ({}) ошибка: {}",
                repo.name,
                combined.trim()
            ));
        }
    }

    let push_out = Command::new(&git_bin)
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-C")
        .arg(&repo.path)
        .arg("push")
        .output()
        .await
        .map_err(|e| format!("Ошибка запуска git push: {e}"))?;

    if !push_out.status.success() {
        let err = String::from_utf8_lossy(&push_out.stderr);
        return Err(format!("git push ({}) ошибка: {}", repo.name, err.trim()));
    }

    if committed {
        Ok(format!(
            "Репозиторий «{}» ({}): коммит и push выполнены.",
            repo.name, repo.branch
        ))
    } else {
        Ok(format!(
            "Репозиторий «{}» ({}): новых изменений нет, push выполнен.",
            repo.name, repo.branch
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_branch_headers() {
        assert_eq!(
            parse_branch_header("## main...origin/main [ahead 2, behind 1]"),
            ("main".to_string(), 2, 1)
        );
        assert_eq!(
            parse_branch_header("## feat/tui"),
            ("feat/tui".to_string(), 0, 0)
        );
        assert_eq!(
            parse_branch_header("## No commits yet on main"),
            ("main".to_string(), 0, 0)
        );
    }

    #[test]
    fn expands_tilde_with_forward_and_back_slashes() {
        let home = home_dir();
        assert_eq!(expand_path("~"), home);
        assert_eq!(expand_path("~/Projects"), home.join("Projects"));
        assert_eq!(expand_path("~\\Projects"), home.join("Projects"));
    }
}
