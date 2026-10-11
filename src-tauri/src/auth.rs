use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

const KEYRING_SERVICE: &str = "com.gitmint.desktop";
const KEYRING_ACCOUNT: &str = "github-access-token";
static TOKEN_CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubUser {
    pub login: String,
    pub avatar_url: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub html_url: String,
    pub clone_url: String,
    pub default_branch: String,
    pub updated_at: String,
    pub fork: bool,
}

#[derive(Deserialize)]
struct GithubRepositoryResponse {
    id: u64,
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
    html_url: String,
    clone_url: String,
    default_branch: String,
    updated_at: String,
    fork: bool,
}

impl From<GithubRepositoryResponse> for GithubRepository {
    fn from(repository: GithubRepositoryResponse) -> Self {
        Self {
            id: repository.id,
            name: repository.name,
            full_name: repository.full_name,
            description: repository.description,
            private: repository.private,
            html_url: repository.html_url,
            clone_url: repository.clone_url,
            default_branch: repository.default_branch,
            updated_at: repository.updated_at,
            fork: repository.fork,
        }
    }
}

#[derive(Deserialize)]
struct GithubProfile {
    login: String,
    avatar_url: String,
    html_url: String,
}

impl From<GithubProfile> for GithubUser {
    fn from(profile: GithubProfile) -> Self {
        Self {
            login: profile.login,
            avatar_url: profile.avatar_url,
            html_url: profile.html_url,
        }
    }
}

#[derive(Deserialize)]
struct BrokerToken {
    access_token: String,
}

#[tauri::command]
pub async fn sign_in_github(
    app: tauri::AppHandle,
    client_id: String,
    broker_url: String,
) -> Result<GithubUser, String> {
    if client_id.trim().is_empty() {
        return Err(
            "GitHub sign-in is not configured. Set VITE_GITHUB_CLIENT_ID and restart GitMint."
                .into(),
        );
    }
    let broker = Url::parse(&broker_url)
        .map_err(|_| "The GitMint sign-in service URL is invalid.".to_string())?;
    if broker.scheme() != "https"
        && !broker
            .host_str()
            .is_some_and(|host| host == "localhost" || host == "127.0.0.1")
    {
        return Err("The GitMint sign-in service must use HTTPS.".into());
    }

    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Could not start GitHub sign-in: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let mut state_bytes = [0u8; 32];
    let mut verifier_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut state_bytes);
    OsRng.fill_bytes(&mut verifier_bytes);
    let state = URL_SAFE_NO_PAD.encode(state_bytes);
    let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));

    let mut authorize = Url::parse("https://github.com/login/oauth/authorize").unwrap();
    authorize
        .query_pairs_mut()
        .append_pair("client_id", client_id.trim())
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", "read:user repo")
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");

    let callback_state = state.clone();
    let callback =
        tauri::async_runtime::spawn_blocking(move || wait_for_callback(listener, &callback_state));
    app.opener()
        .open_url(authorize.as_str(), None::<&str>)
        .map_err(|e| format!("Could not open GitHub in your browser: {e}"))?;
    let code = callback
        .await
        .map_err(|e| format!("GitHub sign-in callback failed: {e}"))??;

    let endpoint = broker.join("/token").map_err(|e| e.to_string())?;
    let response = reqwest::Client::new().post(endpoint)
        .header(USER_AGENT, "GitMint")
        .json(&serde_json::json!({ "code": code, "code_verifier": verifier, "redirect_uri": redirect_uri, "client_id": client_id.trim() }))
        .send().await.map_err(|e| format!("Could not contact GitMint sign-in service: {e}"))?;
    if !response.status().is_success() {
        return Err("GitHub sign-in could not be completed. Check the GitMint sign-in service configuration.".into());
    }
    let token: BrokerToken = response
        .json()
        .await
        .map_err(|_| "GitHub sign-in service returned an invalid response.".to_string())?;
    store_token(&token.access_token)?;
    match fetch_user(&token.access_token).await {
        Ok(user) => Ok(user),
        Err(error) => {
            let _ = clear_token();
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn github_session() -> Result<Option<GithubUser>, String> {
    let token = match read_token() {
        Ok(token) => token,
        Err(_) => return Ok(None),
    };
    match fetch_user(&token).await {
        Ok(user) => Ok(Some(user)),
        Err(_) => {
            let _ = clear_token();
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn list_github_repositories() -> Result<Vec<GithubRepository>, String> {
    let token =
        read_token().map_err(|_| "Sign in with GitHub to view your repositories.".to_string())?;
    let client = reqwest::Client::new();
    let mut repositories = Vec::new();
    for page in 1..=100 {
        let batch: Vec<GithubRepositoryResponse> = client
            .get("https://api.github.com/user/repos")
            .query(&[("sort", "updated"), ("direction", "desc"), ("per_page", "100")])
            .query(&[("page", page)])
            .header(ACCEPT, "application/vnd.github+json")
            .header(USER_AGENT, "GitMint")
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|_| "Could not reach GitHub. Check your internet connection and try again.".to_string())?
            .error_for_status()
            .map_err(|error| {
                if error.status() == Some(reqwest::StatusCode::FORBIDDEN) {
                    "GitHub repository access needs approval. Sign out, then sign in again and approve repository access.".to_string()
                } else {
                    "GitHub could not load your repositories. Please sign in again and retry.".to_string()
                }
            })?
            .json()
            .await
            .map_err(|_| "GitHub returned an invalid repository list.".to_string())?;
        let done = batch.len() < 100;
        repositories.extend(batch.into_iter().map(GithubRepository::from));
        if done {
            break;
        }
    }
    Ok(repositories)
}

#[cfg(test)]
mod tests {
    use super::GithubRepository;

    #[test]
    fn maps_github_repository_json_to_frontend_camel_case() {
        let github_json = r#"{
            "id": 42,
            "name": "gitmint",
            "full_name": "octocat/gitmint",
            "description": "A Git client",
            "private": false,
            "html_url": "https://github.com/octocat/gitmint",
            "clone_url": "https://github.com/octocat/gitmint.git",
            "default_branch": "main",
            "updated_at": "2026-10-11T12:00:00Z",
            "fork": false
        }"#;
        let github: super::GithubRepositoryResponse = serde_json::from_str(github_json).unwrap();
        let frontend = serde_json::to_value(GithubRepository::from(github)).unwrap();
        assert_eq!(frontend["fullName"], "octocat/gitmint");
        assert_eq!(frontend["defaultBranch"], "main");
        assert_eq!(frontend["htmlUrl"], "https://github.com/octocat/gitmint");
    }
}

#[tauri::command]
pub fn sign_out_github() -> Result<(), String> {
    clear_token()
}

#[tauri::command]
pub fn open_github_repository(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let parsed =
        Url::parse(&url).map_err(|_| "That GitHub repository link is invalid.".to_string())?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("github.com") {
        return Err("GitMint can only open repository links on github.com.".into());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|_| "Could not open this repository in your browser.".to_string())
}

pub(crate) fn ensure_github_signed_in() -> Result<(), String> {
    #[cfg(test)]
    return Ok(());
    #[cfg(not(test))]
    read_token()
        .map(|_| ())
        .map_err(|_| "Sign in with GitHub before using GitMint.".to_string())
}

fn wait_for_callback(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => match parse_callback(&mut stream, expected_state) {
                Ok(code) => return Ok(code),
                Err(error) if error == "Continue" => continue,
                Err(error) => return Err(error),
            },
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(80))
            }
            Err(error) => return Err(format!("Could not receive GitHub callback: {error}")),
        }
    }
    Err("GitHub sign-in timed out. Please try again.".into())
}

fn parse_callback(stream: &mut TcpStream, expected_state: &str) -> Result<String, String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buffer = [0u8; 8192];
    let size = stream
        .read(&mut buffer)
        .map_err(|e| format!("Could not read GitHub callback: {e}"))?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let Some(target) = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
    else {
        return Err("Continue".into());
    };
    let url =
        Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| "Continue".to_string())?;
    if url.path() != "/callback" {
        return Err("Continue".into());
    }
    let params = url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    if params.get("state").map(String::as_str) != Some(expected_state) {
        let body = "<!doctype html><meta charset=utf-8><title>GitMint</title><p>Invalid sign-in response. Return to GitMint and try again.</p>";
        let _ = stream.write_all(format!("HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes());
        return Err("GitHub sign-in state did not match. Please try again.".into());
    }
    if let Some(error) = params.get("error") {
        let body = "<!doctype html><meta charset=utf-8><title>GitMint</title><p>GitHub sign-in was cancelled. You can return to GitMint.</p>";
        let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes());
        return Err(format!("GitHub sign-in was cancelled: {error}"));
    }
    let code = params
        .get("code")
        .cloned()
        .ok_or_else(|| "GitHub did not return an authorization code.".to_string())?;
    let body = "<!doctype html><meta charset=utf-8><title>GitMint</title><p>GitMint sign-in completed. You can return to the app.</p>";
    let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes());
    Ok(code)
}

async fn fetch_user(token: &str) -> Result<GithubUser, String> {
    let profile: GithubProfile = reqwest::Client::new()
        .get("https://api.github.com/user")
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "GitMint")
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Could not verify your GitHub account: {e}"))?
        .error_for_status()
        .map_err(|_| "GitHub rejected the saved sign-in. Please sign in again.".to_string())?
        .json()
        .await
        .map_err(|_| "GitHub returned an invalid profile.".to_string())?;
    Ok(profile.into())
}

fn store_token(token: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| e.to_string())?
        .set_password(token)
        .map_err(|e| format!("Could not securely store your GitHub sign-in: {e}"))?;
    cache_token(token.to_string());
    Ok(())
}
fn read_token() -> Result<String, String> {
    if let Ok(cache) = TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock()
        && let Some(token) = cache.as_ref()
    {
        return Ok(token.clone());
    }
    let token = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|e| e.to_string())?;
    cache_token(token.clone());
    Ok(token)
}

pub(crate) fn token_for_git() -> Result<String, String> {
    read_token().map_err(|_| "Sign in with GitHub again to clone this repository.".to_string())
}

fn cache_token(token: String) {
    if let Ok(mut cache) = TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(token);
    }
}
fn clear_token() -> Result<(), String> {
    match keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| e.to_string())?
        .delete_credential()
    {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            if let Ok(mut cache) = TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
                *cache = None;
            }
            Ok(())
        }
        Err(error) => Err(format!(
            "Could not remove GitHub sign-in from Keychain: {error}"
        )),
    }
}
