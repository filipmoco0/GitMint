mod auth;
mod branches;
mod commands;
mod diff;
mod history;
mod operations;
mod repository;
mod status;
mod sync;

pub fn run_git_askpass() -> bool {
    if std::env::var_os("GITMINT_GIT_ASKPASS").is_none() {
        return false;
    }
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !prompt.contains("github.com") {
        return true;
    }
    if prompt.contains("username") {
        println!("x-access-token");
    } else if let Ok(token) = std::env::var("GITMINT_GITHUB_TOKEN") {
        println!("{token}");
    }
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            auth::sign_in_github,
            auth::github_session,
            auth::sign_out_github,
            auth::list_github_repositories,
            auth::search_github_repositories,
            auth::get_github_repository_readme,
            auth::open_github_repository,
            repository::inspect_repository,
            repository::clone_github_repository,
            history::list_commits,
            history::get_commit_details,
            status::list_changes,
            diff::get_file_diff,
            commands::stage_file,
            commands::unstage_file,
            commands::stage_all,
            commands::create_commit,
            branches::list_branches,
            branches::create_branch,
            branches::switch_branch,
            sync::pull_repository,
            sync::push_repository
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
