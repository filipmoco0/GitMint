use crate::status::{list_changes, repository_root, run_git};
use std::{path::Path, process::Command};
use url::Url;

#[tauri::command]
pub fn pull_repository(path: String) -> Result<String, String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    if !list_changes(path.clone())?.is_empty() {
        return Err("Commit or safely move your uncommitted changes before pulling. GitMint will not overwrite local work.".into());
    }

    let upstream = run_git(
        &root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .ok()
    .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string());
    let (remote, branch) = match upstream.as_deref().and_then(|value| value.split_once('/')) {
        Some((remote, branch)) if !remote.is_empty() && !branch.is_empty() => {
            (remote.to_string(), Some(branch.to_string()))
        }
        _ => {
            let branch = run_git(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
                .map_err(|_| "Switch to a named branch before pulling.".to_string())?;
            (
                "origin".to_string(),
                Some(String::from_utf8_lossy(&branch).trim().to_string()),
            )
        }
    };
    ensure_github_remote(&root, &remote)?;
    let Some(branch) = branch else {
        return Err("GitMint could not determine which branch to pull.".into());
    };
    let output = run_authenticated_git(&root, &["pull", "--ff-only", &remote, &branch])?;
    let message = String::from_utf8_lossy(&output);
    if message.contains("Already up to date") || message.contains("Already up-to-date") {
        Ok("Already up to date".into())
    } else {
        Ok("Pull completed".into())
    }
}

#[tauri::command]
pub fn push_repository(path: String) -> Result<String, String> {
    crate::auth::ensure_github_signed_in()?;
    let root = repository_root(&path)?;
    let branch = run_git(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map_err(|_| "Switch to a named branch before pushing.".to_string())?;
    let branch = String::from_utf8_lossy(&branch).trim().to_string();
    let upstream = run_git(
        &root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .is_ok();
    let remote = if upstream {
        let value = run_git(
            &root,
            &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        )?;
        String::from_utf8_lossy(&value)
            .trim()
            .split_once('/')
            .map(|(name, _)| name.to_string())
            .ok_or_else(|| "Git could not identify this branch's remote.".to_string())?
    } else {
        "origin".to_string()
    };
    ensure_github_remote(&root, &remote)?;
    if upstream {
        run_authenticated_git(&root, &["push"])?;
    } else {
        run_authenticated_git(&root, &["push", "--set-upstream", &remote, "HEAD"])?;
    }
    Ok(format!("Pushed {branch}"))
}

#[cfg(test)]
mod tests {
    use super::{ensure_github_remote, pull_repository, push_repository};
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_repo(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gitmint-sync-{label}-{nonce}"));
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
                .arg("-C")
                .arg(&path)
                .args(["config", "user.name", "GitMint Test"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["config", "user.email", "gitmint-test@example.invalid"])
                .status()
                .unwrap()
                .success()
        );
        fs::write(path.join("tracked.txt"), "base\n").unwrap();
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["add", "tracked.txt"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["commit", "--quiet", "-m", "Initial"])
                .status()
                .unwrap()
                .success()
        );
        path
    }

    #[test]
    fn pull_refuses_to_run_when_local_work_exists() {
        let path = temp_repo("dirty-pull");
        fs::write(path.join("tracked.txt"), "local work\n").unwrap();
        let error = pull_repository(path.to_string_lossy().into_owned()).unwrap_err();
        assert!(error.contains("will not overwrite local work"));
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn accepts_only_github_https_remotes_for_sync() {
        let path = temp_repo("github-remote");
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&path)
                .args([
                    "remote",
                    "add",
                    "origin",
                    "https://github.com/example/project.git"
                ])
                .status()
                .unwrap()
                .success()
        );
        assert!(ensure_github_remote(&path, "origin").is_ok());
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&path)
                .args([
                    "remote",
                    "set-url",
                    "origin",
                    "git@github.com:example/project.git"
                ])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            ensure_github_remote(&path, "origin")
                .unwrap_err()
                .contains("GitHub HTTPS remotes")
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn pushes_and_pulls_fast_forward_changes_without_network_access() {
        let local = temp_repo("roundtrip");
        let bare = local.with_extension("bare.git");
        let peer = local.with_extension("peer");
        assert!(
            Command::new("git")
                .args(["init", "--quiet", "--bare", "--initial-branch=main"])
                .arg(&bare)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&local)
                .args([
                    "remote",
                    "add",
                    "origin",
                    "https://github.com/example/project.git"
                ])
                .status()
                .unwrap()
                .success()
        );
        let rewrite_key = format!("url.file://{}.insteadOf", bare.display());
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&local)
                .args([
                    "config",
                    "--local",
                    &rewrite_key,
                    "https://github.com/example/project.git"
                ])
                .status()
                .unwrap()
                .success()
        );

        assert_eq!(
            push_repository(local.to_string_lossy().into_owned()).unwrap(),
            "Pushed main"
        );
        assert!(
            Command::new("git")
                .args(["clone", "--quiet"])
                .arg(&bare)
                .arg(&peer)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&peer)
                .args(["config", "user.name", "GitMint Peer"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&peer)
                .args(["config", "user.email", "peer@example.invalid"])
                .status()
                .unwrap()
                .success()
        );
        fs::write(peer.join("tracked.txt"), "remote update\n").unwrap();
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&peer)
                .args(["add", "tracked.txt"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&peer)
                .args(["commit", "--quiet", "-m", "Remote update"])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&peer)
                .args(["push", "origin", "main"])
                .status()
                .unwrap()
                .success()
        );

        assert_eq!(
            pull_repository(local.to_string_lossy().into_owned()).unwrap(),
            "Pull completed"
        );
        assert_eq!(
            fs::read_to_string(local.join("tracked.txt")).unwrap(),
            "remote update\n"
        );
        fs::remove_dir_all(local).unwrap();
        fs::remove_dir_all(peer).unwrap();
        fs::remove_dir_all(bare).unwrap();
    }
}

fn ensure_github_remote(root: &Path, remote: &str) -> Result<(), String> {
    if remote.is_empty() || remote.starts_with('-') {
        return Err("This branch has an invalid Git remote.".into());
    }
    let url_key = format!("remote.{remote}.url");
    let urls = run_git(root, &["config", "--get-all", &url_key])
        .map_err(|_| format!("GitMint could not find the {remote} remote."))?;
    let urls = String::from_utf8_lossy(&urls);
    if urls.lines().count() != 1 || !is_github_https_url(urls.trim()) {
        return Err("GitMint sync currently supports GitHub HTTPS remotes. Update this remote to an https://github.com link.".into());
    }
    let push_key = format!("remote.{remote}.pushurl");
    if let Ok(push_urls) = run_git(root, &["config", "--get-all", &push_key]) {
        let push_urls = String::from_utf8_lossy(&push_urls);
        if push_urls.lines().count() != 1 || !is_github_https_url(push_urls.trim()) {
            return Err("GitMint sync currently supports GitHub HTTPS remotes. Update this remote to an https://github.com link.".into());
        }
    }
    Ok(())
}

fn is_github_https_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|parsed| {
        parsed.scheme() == "https"
            && parsed.host_str() == Some("github.com")
            && parsed.port().is_none()
            && parsed.username().is_empty()
            && parsed.password().is_none()
    })
}

fn run_authenticated_git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let token = crate::auth::token_for_git()?;
    let askpass = std::env::current_exe()
        .map_err(|_| "GitMint could not prepare secure GitHub access for Git.".to_string())?;
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "credential.helper="])
        .args(args)
        .env("GIT_ASKPASS", askpass)
        .env("GIT_ASKPASS_REQUIRE", "force")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GITMINT_GIT_ASKPASS", "1")
        .env("GITMINT_GITHUB_TOKEN", token)
        .output()
        .map_err(|_| "Could not start Git to sync this repository.".to_string())?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let lower = detail.to_ascii_lowercase();
    if lower.contains("authentication failed")
        || lower.contains("repository not found")
        || lower.contains("could not read username")
    {
        Err(
            "GitHub did not allow this operation. Check your repository access and sign in again."
                .into(),
        )
    } else if lower.contains("not possible to fast-forward") || lower.contains("divergent branches")
    {
        Err("This branch has diverged from GitHub. GitMint only pulls fast-forward changes to protect your work; resolve the branch history on GitHub or with Git, then retry.".into())
    } else if detail.is_empty() {
        Err("Git could not sync this repository. Check your connection and try again.".into())
    } else {
        Err(format!("Git could not sync this repository: {detail}"))
    }
}
