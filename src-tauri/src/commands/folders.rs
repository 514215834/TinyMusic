use std::path::{Path, PathBuf};

use rusqlite::params;
use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::error::{AppError, AppErrorDto, AppResult};
use crate::library;
use crate::library::scanner::AUDIO_EXTS;
use crate::models::{DropImportResult, Folder, Track};
use crate::AppState;

use super::tracks::{row_to_track, TRACK_SELECT};

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

/// 候选目录是否已被某个根目录覆盖（相等或为其子目录）；比较用小写+反斜杠归一化（同 m3u8 路径匹配）
fn is_covered(path: &str, roots: impl IntoIterator<Item = String>) -> bool {
    let path = library::m3u8::normalize_path(path);
    roots.into_iter().any(|root| {
        let root = library::m3u8::normalize_path(&root);
        path == root || path.starts_with(&format!("{root}\\"))
    })
}

/// 拖拽导入（M7）：目录直接作为曲库来源；音频文件取其所在目录——曲库以目录为管理单元
/// （tracks.folder_id 非空 + 增量监听按目录），单文件导入无法纳入监听与删除联动。
/// 已被现有曲库目录或本次候选覆盖的范围跳过，避免重复扫描。
#[tauri::command]
#[specta::specta]
pub async fn drop_import(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<DropImportResult, AppErrorDto> {
    let mut skipped: u32 = 0;

    // 目录本身 / 音频文件的父目录 → 候选；不支持的路径计数跳过
    let mut candidates: Vec<PathBuf> = Vec::new();
    for raw in paths {
        let p = PathBuf::from(raw);
        let candidate = if p.is_dir() {
            Some(p)
        } else if p.is_file()
            && p.extension()
                .is_some_and(|e| AUDIO_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str()))
        {
            p.parent().map(Path::to_path_buf)
        } else {
            None
        };
        match candidate {
            Some(c) => candidates.push(c),
            None => skipped += 1,
        }
    }
    // 父目录排前：嵌套候选先命中父级，子目录随后按覆盖检查跳过
    candidates.sort();
    candidates.dedup();

    let mut roots: Vec<String> = {
        let conn = state.conn.lock();
        db::folder_rows(&conn)?.into_iter().map(|(_, path)| path).collect()
    };

    let mut added = 0u32;
    for candidate in candidates {
        let Ok(canonical) = candidate.canonicalize() else {
            skipped += 1;
            continue;
        };
        let path_str = normalize_path(&canonical);
        if is_covered(&path_str, roots.iter().cloned()) {
            skipped += 1;
            continue;
        }
        add_folder(&app, &state, &path_str).map_err(AppErrorDto::from)?;
        roots.push(path_str);
        added += 1;
    }

    let result = DropImportResult { folders_added: added, folders_skipped: skipped };
    if result.folders_added > 0 {
        library::spawn_scan(app);
    }
    Ok(result)
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

/// 文件夹视图（M4）：某来源目录下的全部曲目（含子目录），按路径排序呈现目录结构
fn folder_track_list(state: &AppState, folder_id: i32) -> AppResult<Vec<Track>> {
    let conn = state.conn.lock();
    let sql = format!("{TRACK_SELECT} WHERE t.folder_id = ?1 ORDER BY t.path");
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params![i64::from(folder_id)], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub async fn folder_tracks(
    state: State<'_, AppState>,
    folder_id: i32,
) -> Result<Vec<Track>, AppErrorDto> {
    folder_track_list(&state, folder_id).map_err(AppErrorDto::from)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{is_covered, normalize_path};

    #[test]
    fn 剥离扩展路径前缀() {
        assert_eq!(normalize_path(&PathBuf::from(r"\\?\G:\Music")), r"G:\Music");
        assert_eq!(normalize_path(&PathBuf::from(r"\\?\UNC\server\share")), r"\\server\share");
        assert_eq!(normalize_path(&PathBuf::from(r"G:\Music")), r"G:\Music");
    }

    #[test]
    fn 覆盖检查忽略大小写且要求完整路径段() {
        let roots = vec![r"G:\Music".to_string()];
        assert!(is_covered(r"G:\Music", roots.clone()));
        assert!(is_covered(r"g:\music\子目录\歌.flac", roots.clone()));
        assert!(!is_covered(r"G:\MusicB", roots.clone()), "前缀相同但不是同一目录段");
        assert!(!is_covered(r"G:\Other", roots));
    }
}
