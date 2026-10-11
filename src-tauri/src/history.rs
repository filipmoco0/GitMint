use serde::Serialize;
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub authored_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub authored_at: String,
    pub files: Vec<String>,
}

#[tauri::command]
pub fn list_commits(path: String) -> Result<Vec<Commit>, String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    if run_git(&root, &["rev-parse", "--verify", "HEAD"]).is_err() {
        return Ok(Vec::new());
    }
    let output = run_git(
        &root,
        &[
            "log",
            "-n",
            "100",
            "--date=iso-strict",
            "--pretty=format:%H%x1f%h%x1f%an%x1f%aI%x1f%s%x1e",
        ],
    )?;

    Ok(output
        .split('\x1e')
        .filter_map(|record| {
            let fields: Vec<&str> = record.trim_matches(['\n', '\r']).split('\x1f').collect();
            (fields.len() == 5 && !fields[0].is_empty()).then(|| Commit {
                hash: fields[0].to_string(),
                short_hash: fields[1].to_string(),
                author: fields[2].to_string(),
                authored_at: fields[3].to_string(),
                message: fields[4].to_string(),
            })
        })
        .collect())
}

#[tauri::command]
pub fn get_commit_details(path: String, hash: String) -> Result<CommitDetails, String> {
    crate::auth::ensure_github_signed_in()?;
    if hash.is_empty() || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Choose a valid commit.".into());
    }
    let root = repository_root(&path)?;
    let metadata = run_git(
        &root,
        &["show", "-s", "--format=%H%x1f%an%x1f%aI%x1f%B", &hash],
    )?;
    let mut fields = metadata.splitn(4, '\x1f');
    let resolved_hash = fields.next().unwrap_or_default().trim().to_string();
    let author = fields.next().unwrap_or_default().to_string();
    let authored_at = fields.next().unwrap_or_default().to_string();
    let message = fields.next().unwrap_or_default().trim().to_string();
    if resolved_hash.is_empty() {
        return Err("That commit could not be found in this repository.".into());
    }

    let file_output = run_git(
        &root,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "--root",
            &hash,
        ],
    )?;
    let files = file_output
        .lines()
        .filter(|file| !file.is_empty())
        .map(str::to_string)
        .collect();

    Ok(CommitDetails {
        hash: resolved_hash,
        message,
        author,
        authored_at,
        files,
    })
}

fn repository_root(path: &str) -> Result<std::path::PathBuf, String> {
    let selected = Path::new(path.trim());
    let selected = selected
        .canonicalize()
        .map_err(|_| "That folder does not exist or cannot be accessed.".to_string())?;
    if !selected.is_dir() {
        return Err("The selected path is not a folder.".into());
    }
    let root = run_git(&selected, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "This folder is not a Git repository.".to_string())?;
    Path::new(root.trim())
        .canonicalize()
        .map_err(|_| "Git returned an invalid repository path.".to_string())
}

fn run_git(path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|error| format!("Could not run Git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    String::from_utf8(output.stdout).map_err(|_| "Git returned invalid text.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{get_commit_details, list_commits};
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
            std::env::temp_dir().join(format!("gitmint-history-{}-{nonce}", std::process::id()));
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
        fs::write(path.join("README.md"), "hello\n").unwrap();
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["add", "README.md"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["-C"])
                .arg(&path)
                .args(["commit", "-m", "Add readme"])
                .status()
                .unwrap()
                .success()
        );
        path
    }

    #[test]
    fn lists_commits_and_reads_selected_commit_files() {
        let path = temp_repo();
        let commits = list_commits(path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Add readme");
        assert_eq!(commits[0].author, "Test Author");

        let details =
            get_commit_details(path.to_string_lossy().into_owned(), commits[0].hash.clone())
                .unwrap();
        assert_eq!(details.message, "Add readme");
        assert_eq!(details.files, vec!["README.md"]);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn empty_repository_has_no_commits() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gitmint-history-empty-{}-{nonce}",
            std::process::id()
        ));
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
            list_commits(path.to_string_lossy().into_owned())
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(path).unwrap();
    }
}
