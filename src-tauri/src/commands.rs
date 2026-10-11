use crate::operations;

#[tauri::command]
pub fn stage_file(path: String, file_path: String) -> Result<(), String> {
    crate::auth::ensure_github_signed_in()?;
    operations::stage_file(&path, &file_path)
}

#[tauri::command]
pub fn unstage_file(path: String, file_path: String) -> Result<(), String> {
    crate::auth::ensure_github_signed_in()?;
    operations::unstage_file(&path, &file_path)
}

#[tauri::command]
pub fn stage_all(path: String) -> Result<(), String> {
    crate::auth::ensure_github_signed_in()?;
    operations::stage_all(&path)
}

#[tauri::command]
pub fn create_commit(path: String, message: String) -> Result<String, String> {
    crate::auth::ensure_github_signed_in()?;
    operations::create_commit(&path, &message)
}
