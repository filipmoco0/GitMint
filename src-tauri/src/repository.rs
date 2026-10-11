use serde::Serialize;
use std::{path::PathBuf, process::Command};
use url::Url;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInfo {
    pub path: String,
    pub name: String,
    pub branch: String,
}

#[tauri::command]
pub fn inspect_repository(path: String) -> Result<RepositoryInfo, String> {
    crate::auth::ensure_github_signed_in()?;
    let requested_path = PathBuf::from(path.trim());
    if requested_path.as_os_str().is_empty() {
        return Err("Choose a folder to open.".into());
    }

    let canonical_path = requested_path
        .canonicalize()
        .map_err(|_| "That folder does not exist or cannot be accessed.".to_string())?;
    if !canonical_path.is_dir() {
        return Err("The selected path is not a folder.".into());
    }

    let root = git_output(&canonical_path, ["rev-parse", "--show-toplevel"])
        .map_err(|_| "This folder is not a Git repository.".to_string())?;
    let root_path = PathBuf::from(root.trim());
    let root_path = root_path
        .canonicalize()
        .map_err(|_| "Git returned an invalid repository path.".to_string())?;

    let branch = git_output(&root_path, ["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "Detached HEAD".to_string());
    let name = root_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Repository")
        .to_string();

    Ok(RepositoryInfo {
        path: root_path.to_string_lossy().into_owned(),
        name,
        branch,
    })
}

#[tauri::command]
pub fn clone_github_repository(
    clone_url: String,
    parent_path: String,
) -> Result<RepositoryInfo, String> {
    crate::auth::ensure_github_signed_in()?;
    let parsed =
        Url::parse(&clone_url).map_err(|_| "That GitHub clone link is invalid.".to_string())?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("GitMint can only clone repositories from github.com over HTTPS.".into());
    }
    let segments = parsed
        .path_segments()
        .ok_or_else(|| "That GitHub clone link is invalid.".to_string())?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() != 2 {
        return Err("That GitHub clone link does not point to one repository.".into());
    }
    let repo_name = segments[1].strip_suffix(".git").unwrap_or(segments[1]);
    if repo_name.is_empty()
        || !repo_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("That GitHub repository name cannot be used as a folder name.".into());
    }

    let parent = PathBuf::from(parent_path.trim())
        .canonicalize()
        .map_err(|_| "Choose a folder that exists and can be accessed.".to_string())?;
    if !parent.is_dir() {
        return Err("Choose a folder that exists and can be accessed.".into());
    }
    let destination = parent.join(repo_name);
    if std::fs::symlink_metadata(&destination).is_ok() {
        return Err(format!(
            "A folder named {repo_name} already exists there. Choose another parent folder."
        ));
    }

    let token = crate::auth::token_for_git()?;
    let askpass = std::env::current_exe()
        .map_err(|_| "GitMint could not prepare secure GitHub access for Git.".to_string())?;
    let output = Command::new("git")
        .current_dir(&parent)
        .args([
            "-c",
            "credential.helper=",
            "clone",
            "--progress",
            "--",
            &clone_url,
        ])
        .arg(&destination)
        .env("GIT_ASKPASS", askpass)
        .env("GIT_ASKPASS_REQUIRE", "force")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GITMINT_GIT_ASKPASS", "1")
        .env("GITMINT_GITHUB_TOKEN", token)
        .output()
        .map_err(|_| "Could not start Git to clone this repository.".to_string())?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        let lower = detail.to_ascii_lowercase();
        if lower.contains("authentication failed")
            || lower.contains("repository not found")
            || lower.contains("could not read username")
        {
            return Err("GitHub would not allow this clone. Check that your account has access, then sign in again.".into());
        }
        return Err(if detail.is_empty() {
            "Git could not clone this repository. Check your connection and try again.".into()
        } else {
            format!("Git could not clone this repository: {detail}")
        });
    }

    inspect_repository(destination.to_string_lossy().into_owned())
}

fn git_output<const N: usize>(path: &std::path::Path, args: [&str; N]) -> Result<String, String> {
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
    use super::inspect_repository;
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("gitmint-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn opens_an_empty_git_repository_and_reports_branch() {
        let path = temp_dir("empty-repo");
        let init = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .arg("--initial-branch=main")
            .arg(&path)
            .status()
            .unwrap();
        assert!(init.success());

        let repo = inspect_repository(path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(repo.name, path.file_name().unwrap().to_string_lossy());
        assert_eq!(repo.branch, "main");
        assert_eq!(PathBuf::from(&repo.path), path.canonicalize().unwrap());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rejects_a_non_repository_folder() {
        let path = temp_dir("not-repo");
        let result = inspect_repository(path.to_string_lossy().into_owned());
        assert_eq!(result.unwrap_err(), "This folder is not a Git repository.");
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rejects_a_missing_path() {
        let path = temp_dir("missing");
        fs::remove_dir_all(&path).unwrap();
        assert_eq!(
            inspect_repository(path.to_string_lossy().into_owned()).unwrap_err(),
            "That folder does not exist or cannot be accessed."
        );
    }
}
