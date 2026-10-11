use crate::status::{list_changes, repository_root, run_git};
use serde::Serialize;
use std::{
    fs,
    path::{Component, Path},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub staged: bool,
    pub text: String,
}

#[tauri::command]
pub fn get_file_diff(path: String, file_path: String, staged: bool) -> Result<FileDiff, String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    let current_changes = list_changes(path)?;
    let change = current_changes
        .iter()
        .find(|change| change.path == file_path)
        .ok_or_else(|| "That file is no longer changed in this repository.".to_string())?;

    if staged && change.staged_status.is_none() {
        return Err("That file has no staged changes.".into());
    }
    if !staged && change.unstaged_status.is_none() {
        return Err("That file has no unstaged changes.".into());
    }

    let text = if !staged && change.unstaged_status.as_deref() == Some("Untracked") {
        untracked_diff(&root, &file_path)?
    } else {
        let args = if staged {
            vec![
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "--",
                &file_path,
            ]
        } else {
            vec![
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "--",
                &file_path,
            ]
        };
        String::from_utf8_lossy(&run_git(&root, &args)?).into_owned()
    };

    Ok(FileDiff {
        path: file_path,
        staged,
        text,
    })
}

fn untracked_diff(root: &Path, file_path: &str) -> Result<String, String> {
    let relative = Path::new(file_path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("That file path is outside the repository.".into());
    }
    let absolute = root.join(relative);
    let metadata = fs::symlink_metadata(&absolute)
        .map_err(|_| "That file is no longer available.".to_string())?;
    if !metadata.file_type().is_file() {
        return Err("Diff preview is unavailable for this file type.".into());
    }
    let contents = fs::read(&absolute).map_err(|_| "Could not read this file.".to_string())?;
    let contents = String::from_utf8(contents)
        .map_err(|_| "Binary file; text preview is unavailable.".to_string())?;
    let mut diff = format!(
        "diff --git a/{file_path} b/{file_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{file_path}\n"
    );
    for line in contents.split_inclusive('\n') {
        diff.push('+');
        diff.push_str(line);
        if !line.ends_with('\n') {
            diff.push('\n');
        }
    }
    Ok(diff)
}

#[cfg(test)]
mod tests {
    use super::get_file_diff;
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
            std::env::temp_dir().join(format!("gitmint-diff-{}-{nonce}", std::process::id()));
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
    fn reads_unstaged_and_untracked_diffs() {
        let path = temp_repo();
        fs::write(path.join("tracked.txt"), "changed\n").unwrap();
        fs::write(path.join("new.txt"), "new line\n").unwrap();
        let tracked = get_file_diff(
            path.to_string_lossy().into_owned(),
            "tracked.txt".into(),
            false,
        )
        .unwrap();
        let new_file =
            get_file_diff(path.to_string_lossy().into_owned(), "new.txt".into(), false).unwrap();
        assert!(tracked.text.contains("-original"));
        assert!(tracked.text.contains("+changed"));
        assert!(new_file.text.contains("+new line"));
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn reads_staged_diff_and_rejects_a_non_changed_path() {
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
        let staged = get_file_diff(
            path.to_string_lossy().into_owned(),
            "tracked.txt".into(),
            true,
        )
        .unwrap();
        assert!(staged.text.contains("+staged"));
        assert!(
            get_file_diff(
                path.to_string_lossy().into_owned(),
                "../outside.txt".into(),
                false
            )
            .is_err()
        );
        fs::remove_dir_all(path).unwrap();
    }
}
