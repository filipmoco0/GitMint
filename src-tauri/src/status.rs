use serde::Serialize;
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub staged_status: Option<String>,
    pub unstaged_status: Option<String>,
}

#[tauri::command]
pub fn list_changes(path: String) -> Result<Vec<ChangedFile>, String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    let output = run_git(
        &root,
        &["status", "--porcelain=v1", "--untracked-files=all", "-z"],
    )?;
    let mut records = output.split(|byte| *byte == 0);
    let mut files = Vec::new();

    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let bytes = record;
        if bytes.len() < 4 {
            continue;
        }
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let file_path = String::from_utf8_lossy(&bytes[3..]).into_owned();
        if x == 'R' || x == 'C' || y == 'R' || y == 'C' {
            let _ = records.next(); // The following NUL field is the old path.
        }
        let untracked = x == '?' && y == '?';
        let staged_status = if x != ' ' && x != '?' {
            Some(status_label(x, untracked))
        } else {
            None
        };
        let unstaged_status = if y != ' ' || untracked {
            Some(status_label(if untracked { '?' } else { y }, untracked))
        } else {
            None
        };
        files.push(ChangedFile {
            path: file_path,
            staged_status,
            unstaged_status,
        });
    }

    Ok(files)
}

fn status_label(code: char, untracked: bool) -> String {
    if untracked {
        "Untracked".into()
    } else {
        match code {
            'A' => "Added",
            'D' => "Deleted",
            'R' => "Renamed",
            'C' => "Copied",
            'U' => "Conflicted",
            _ => "Modified",
        }
        .into()
    }
}

pub(crate) fn repository_root(path: &str) -> Result<std::path::PathBuf, String> {
    let selected = Path::new(path.trim())
        .canonicalize()
        .map_err(|_| "That folder does not exist or cannot be accessed.".to_string())?;
    if !selected.is_dir() {
        return Err("The selected path is not a folder.".into());
    }
    let root = run_git(&selected, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "This folder is not a Git repository.".to_string())?;
    Path::new(String::from_utf8_lossy(&root).trim())
        .canonicalize()
        .map_err(|_| "Git returned an invalid repository path.".to_string())
}

pub(crate) fn run_git(path: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|error| format!("Could not run Git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::list_changes;
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
        let path =
            std::env::temp_dir().join(format!("gitmint-status-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["config", "user.name", "Test Author"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["config", "user.email", "test@example.invalid"])
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
    fn reports_unstaged_and_untracked_files() {
        let path = temp_repo();
        fs::write(path.join("tracked.txt"), "changed\n").unwrap();
        fs::write(path.join("new.txt"), "new\n").unwrap();
        let files = list_changes(path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|file| file.path == "tracked.txt"
            && file.unstaged_status.as_deref() == Some("Modified")));
        assert!(
            files.iter().any(|file| file.path == "new.txt"
                && file.unstaged_status.as_deref() == Some("Untracked"))
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn reports_staged_and_unstaged_sides_separately() {
        let path = temp_repo();
        fs::write(path.join("tracked.txt"), "staged\n").unwrap();
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["add", "tracked.txt"])
                .status()
                .unwrap()
                .success()
        );
        fs::write(path.join("tracked.txt"), "staged and then changed\n").unwrap();
        let files = list_changes(path.to_string_lossy().into_owned()).unwrap();
        let tracked = files
            .iter()
            .find(|file| file.path == "tracked.txt")
            .unwrap();
        assert_eq!(tracked.staged_status.as_deref(), Some("Modified"));
        assert_eq!(tracked.unstaged_status.as_deref(), Some("Modified"));
        fs::remove_dir_all(path).unwrap();
    }
}
