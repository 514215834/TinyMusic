use std::path::{Path, PathBuf};

use rusqlite::params;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppErrorDto, AppResult};
use crate::library;
use crate::models::Folder;
use crate::AppState;

/// 剥离 canonicalize 产生的扩展路径前缀（\\?\ 与 \\?\UNC\），存储与 asset 协议统一用普通路径
fn normalize_path(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s
    }
}

fn add_folder(app: &AppHandle, state: &AppState, path: &str) -> AppResult<Folder> {
    let p = PathBuf::from(path);
    if !p.is_dir() {
        return Err(AppError::Message(format!("目录不存在或不可访问: {path}")));
    }
    let path_str = normalize_path(&p.canonicalize()?);

    let folder = {
        let conn = state.conn.lock();
        conn.execute("INSERT OR IGNORE INTO folders(path) VALUES (?1)", params![path_str])?;
        let id: i64 =
            conn.query_row("SELECT id FROM folders WHERE path = ?1", params![path_str], |r| r.get(0))?;
        Folder { id: id as i32, path: path_str.clone() }
    };

    // 新目录纳入 asset 协议范围（音频/封面经 convertFileSrc 访问）
    app.asset_protocol_scope().allow_directory(std::path::Path::new(&path_str), true)?;
    // 新目录纳入增量监听（文件增删改自动同步曲库）
    state.watcher.watch(std::path::Path::new(&path_str));
    Ok(folder)
}

#[tauri::command]
#[specta::specta]
pub async fn folder_add(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Folder, AppErrorDto> {
    let folder = add_folder(&app, &state, &path).map_err(AppErrorDto::from)?;
    library::spawn_scan(app);
    Ok(folder)
}

fn list_folders(state: &AppState) -> AppResult<Vec<Folder>> {
    let conn = state.conn.lock();
    let mut stmt = conn.prepare("SELECT id, path FROM folders ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok(Folder { id: r.get::<_, i64>(0)? as i32, path: r.get(1)? })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
#[specta::specta]
pub async fn folder_list(state: State<'_, AppState>) -> Result<Vec<Folder>, AppErrorDto> {
    list_folders(&state).map_err(AppErrorDto::from)
}

/// 删除文件夹：tracks 经外键 CASCADE 一并删除（foreign_keys=ON）；
/// 同时取消该目录的增量监听
fn remove_folder(state: &AppState, id: i32) -> AppResult<bool> {
    let conn = state.conn.lock();
    let path: Option<String> = conn
        .query_row(
            "SELECT path FROM folders WHERE id = ?1",
            params![i64::from(id)],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    let n = conn.execute("DELETE FROM folders WHERE id = ?1", params![i64::from(id)])?;
    if let Some(path) = path {
        state.watcher.unwatch(std::path::Path::new(&path));
    }
    Ok(n > 0)
}

#[tauri::command]
#[specta::specta]
pub async fn folder_remove(state: State<'_, AppState>, id: i32) -> Result<bool, AppErrorDto> {
    remove_folder(&state, id).map_err(AppErrorDto::from)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::normalize_path;

    #[test]
    fn 剥离扩展路径前缀() {
        assert_eq!(normalize_path(&PathBuf::from(r"\\?\G:\Music")), r"G:\Music");
        assert_eq!(normalize_path(&PathBuf::from(r"\\?\UNC\server\share")), r"\\server\share");
        assert_eq!(normalize_path(&PathBuf::from(r"G:\Music")), r"G:\Music");
    }
}
