use crate::status::{list_changes, repository_root, run_git};

pub fn stage_file(path: &str, file_path: &str) -> Result<(), String> {
    let root = repository_root(path)?;
    let file = changed_file(path, file_path)?;
    if file.unstaged_status.is_none() {
        return Err("That file has no unstaged changes to stage.".into());
    }
    run_git(&root, &["add", "--", file_path]).map(|_| ())
}

pub fn unstage_file(path: &str, file_path: &str) -> Result<(), String> {
    let root = repository_root(path)?;
    let file = changed_file(path, file_path)?;
    if file.staged_status.is_none() {
        return Err("That file has no staged changes to unstage.".into());
    }

    if run_git(&root, &["rev-parse", "--verify", "HEAD"]).is_err() {
        // In an unborn repository every staged path is new; removing it from
        // the index leaves the working file in place.
        run_git(&root, &["rm", "--cached", "-f", "--", file_path]).map(|_| ())
    } else {
        run_git(&root, &["restore", "--staged", "--", file_path]).map(|_| ())
    }
}

pub fn stage_all(path: &str) -> Result<(), String> {
    let root = repository_root(path)?;
    run_git(&root, &["add", "-A"]).map(|_| ())
}

pub fn create_commit(path: &str, message: &str) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Enter a commit message first.".into());
    }
    let root = repository_root(path)?;
    if !list_changes(path.to_string())?
        .iter()
        .any(|file| file.staged_status.is_some())
    {
        return Err("Stage at least one change before committing.".into());
    }
    run_git(&root, &["commit", "-m", message]).map_err(|error| {
        if error.contains("Author identity unknown") || error.contains("Please tell me who you are") {
            "Git needs your author identity. Set git config user.name and user.email, then try again.".to_string()
        } else {
            error
        }
    })?;
    let hash = run_git(&root, &["rev-parse", "--short", "HEAD"])?;
    Ok(String::from_utf8_lossy(&hash).trim().to_string())
}

fn changed_file(path: &str, file_path: &str) -> Result<crate::status::ChangedFile, String> {
    list_changes(path.to_string())?
        .into_iter()
        .find(|file| file.path == file_path)
        .ok_or_else(|| "That file is no longer changed in this repository.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{create_commit, stage_all, stage_file, unstage_file};
    use crate::status::list_changes;
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_repo() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gitmint-ops-{}-{nonce}", std::process::id()));
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
    fn stages_and_unstages_one_file_without_losing_working_content() {
        let path = temp_repo();
        fs::write(path.join("tracked.txt"), "edited\n").unwrap();
        let repo = path.to_string_lossy().into_owned();
        stage_file(&repo, "tracked.txt").unwrap();
        assert!(
            list_changes(repo.clone()).unwrap()[0]
                .staged_status
                .is_some()
        );
        unstage_file(&repo, "tracked.txt").unwrap();
        let file = &list_changes(repo).unwrap()[0];
        assert!(file.staged_status.is_none());
        assert!(file.unstaged_status.is_some());
        assert_eq!(
            fs::read_to_string(path.join("tracked.txt")).unwrap(),
            "edited\n"
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn stages_all_changes_and_creates_a_commit() {
        let path = temp_repo();
        fs::write(path.join("tracked.txt"), "updated\n").unwrap();
        fs::write(path.join("new.txt"), "new\n").unwrap();
        let repo = path.to_string_lossy().into_owned();
        stage_all(&repo).unwrap();
        assert!(
            list_changes(repo.clone())
                .unwrap()
                .iter()
                .all(|file| file.staged_status.is_some())
        );
        let hash = create_commit(&repo, "  Add a test change  ").unwrap();
        assert!(!hash.is_empty());
        assert!(list_changes(repo).unwrap().is_empty());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rejects_empty_commit_messages_and_commits_without_staged_changes() {
        let path = temp_repo();
        let repo = path.to_string_lossy().into_owned();
        assert_eq!(
            create_commit(&repo, "  ").unwrap_err(),
            "Enter a commit message first."
        );
        assert_eq!(
            create_commit(&repo, "No staged changes").unwrap_err(),
            "Stage at least one change before committing."
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn can_unstage_and_commit_the_first_files_in_an_empty_repository() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("gitmint-unborn-{}-{nonce}", std::process::id()));
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
        fs::write(path.join("first.txt"), "first file\n").unwrap();
        let repo = path.to_string_lossy().into_owned();
        stage_file(&repo, "first.txt").unwrap();
        unstage_file(&repo, "first.txt").unwrap();
        assert_eq!(
            fs::read_to_string(path.join("first.txt")).unwrap(),
            "first file\n"
        );
        stage_all(&repo).unwrap();
        assert!(!create_commit(&repo, "First commit").unwrap().is_empty());
        assert!(list_changes(repo).unwrap().is_empty());
        fs::remove_dir_all(path).unwrap();
    }
}
