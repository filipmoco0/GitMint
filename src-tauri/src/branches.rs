use crate::status::{list_changes, repository_root, run_git};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub current: bool,
}

#[tauri::command]
pub fn list_branches(path: String) -> Result<Vec<Branch>, String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    let output = run_git(&root, &["branch", "--format=%(refname:short)%09%(HEAD)"])?;
    Ok(String::from_utf8_lossy(&output)
        .lines()
        .filter_map(|line| {
            let (name, marker) = line.split_once('\t')?;
            (!name.is_empty()).then(|| Branch {
                name: name.to_string(),
                current: marker.trim() == "*",
            })
        })
        .collect())
}

#[tauri::command]
pub fn create_branch(path: String, name: String) -> Result<(), String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    let name = name.trim();
    validate_branch_name(&root, name)?;
    run_git(&root, &["branch", name]).map(|_| ())
}

#[tauri::command]
pub fn switch_branch(path: String, name: String) -> Result<String, String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    let name = name.trim();
    let branches = list_branches(path.clone())?;
    if !branches.iter().any(|branch| branch.name == name) {
        return Err("That local branch does not exist.".into());
    }
    let current = branches
        .iter()
        .find(|branch| branch.current)
        .map(|branch| branch.name.as_str());
    if current == Some(name) {
        return Ok(name.to_string());
    }

    let changes = list_changes(path)?;
    if !changes.is_empty() {
        return Err(format!(
            "Switching branches is paused because this repository has {} changed file(s). Commit or safely move your work first; GitMint will not discard it.",
            changes.len()
        ));
    }

    run_git(&root, &["switch", "--no-guess", name])?;
    Ok(name.to_string())
}

fn validate_branch_name(root: &std::path::Path, name: &str) -> Result<(), String> {
    if name.is_empty() || name.starts_with('-') {
        return Err("Enter a valid branch name.".into());
    }
    run_git(root, &["check-ref-format", "--branch", name])
        .map_err(|_| "That branch name is not valid.".to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create_branch, list_branches, switch_branch};
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_REPO_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = TEMP_REPO_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "gitmint-branches-{}-{nonce}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet", "--initial-branch=main"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["config", "user.name", "GitMint Test"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["config", "user.email", "gitmint-test@example.invalid"])
                .status()
                .unwrap()
                .success()
        );
        fs::write(path.join("tracked.txt"), "original\n").unwrap();
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["add", "tracked.txt"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["commit", "-m", "Initial"])
                .status()
                .unwrap()
                .success()
        );
        path
    }

    #[test]
    fn lists_and_creates_local_branches() {
        let path = temp_repo();
        let repo = path.to_string_lossy().into_owned();
        create_branch(repo.clone(), "feature/ui".into()).unwrap();
        let branches = list_branches(repo).unwrap();
        assert!(
            branches
                .iter()
                .any(|branch| branch.name == "main" && branch.current)
        );
        assert!(
            branches
                .iter()
                .any(|branch| branch.name == "feature/ui" && !branch.current)
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rejects_invalid_branch_names() {
        let path = temp_repo();
        let error =
            create_branch(path.to_string_lossy().into_owned(), "bad name".into()).unwrap_err();
        assert_eq!(error, "That branch name is not valid.");
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn blocks_switching_with_modified_and_untracked_changes() {
        let path = temp_repo();
        let repo = path.to_string_lossy().into_owned();
        create_branch(repo.clone(), "feature".into()).unwrap();
        fs::write(path.join("tracked.txt"), "modified\n").unwrap();
        let error = switch_branch(repo.clone(), "feature".into()).unwrap_err();
        assert!(error.contains("1 changed file"));
        assert_eq!(
            fs::read_to_string(path.join("tracked.txt")).unwrap(),
            "modified\n"
        );
        fs::write(path.join("tracked.txt"), "original\n").unwrap();
        fs::write(path.join("tracked.txt"), "staged edit\n").unwrap();
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["add", "tracked.txt"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            switch_branch(repo.clone(), "feature".into())
                .unwrap_err()
                .contains("1 changed file")
        );
        assert_eq!(
            fs::read_to_string(path.join("tracked.txt")).unwrap(),
            "staged edit\n"
        );
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["restore", "--staged", "--", "tracked.txt"])
                .status()
                .unwrap()
                .success()
        );
        fs::write(path.join("tracked.txt"), "original\n").unwrap();
        fs::write(path.join("new.txt"), "untracked\n").unwrap();
        assert!(
            switch_branch(repo, "feature".into())
                .unwrap_err()
                .contains("1 changed file")
        );
        assert_eq!(
            fs::read_to_string(path.join("new.txt")).unwrap(),
            "untracked\n"
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn switches_clean_working_tree() {
        let path = temp_repo();
        let repo = path.to_string_lossy().into_owned();
        create_branch(repo.clone(), "feature".into()).unwrap();
        assert_eq!(
            switch_branch(repo.clone(), "feature".into()).unwrap(),
            "feature"
        );
        assert!(
            list_branches(repo)
                .unwrap()
                .iter()
                .any(|branch| branch.name == "feature" && branch.current)
        );
        fs::remove_dir_all(path).unwrap();
    }
}
