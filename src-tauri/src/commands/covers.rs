use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::AppState;

/// 返回封面缓存文件的绝对路径（前端用 convertFileSrc 转 asset URL）
fn resolve_cover_path(state: &AppState, file: &str) -> AppResult<Option<String>> {
    // 只接受纯文件名，防路径穿越
    if file.is_empty() || file.contains('/') || file.contains('\\') || file.contains("..") {
        return Ok(None);
    }
    let path = crate::library::covers::path_of(&state.covers_dir, file);
    Ok(path.exists().then(|| path.to_string_lossy().to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn cover_path(state: State<'_, AppState>, file: String) -> Result<Option<String>, AppErrorDto> {
    resolve_cover_path(&state, &file).map_err(AppErrorDto::from)
}
