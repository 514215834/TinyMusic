use rusqlite::params;
use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::models::Track;
use crate::AppState;

use super::tracks::{row_to_track, TRACK_SELECT};

/// 切换收藏状态，返回切换后是否已收藏
fn toggle(state: &AppState, track_id: i32) -> AppResult<bool> {
    let conn = state.conn.lock();
    let deleted = conn.execute(
        "DELETE FROM favorites WHERE track_id = ?1",
        params![i64::from(track_id)],
    )?;
    if deleted > 0 {
        return Ok(false);
    }
    // 曲目不存在时外键约束报错，转成业务错误
    conn.execute(
        "INSERT INTO favorites(track_id) VALUES (?1)",
        params![i64::from(track_id)],
    )?;
    Ok(true)
}

#[tauri::command]
#[specta::specta]
pub async fn favorite_toggle(
    state: State<'_, AppState>,
    track_id: i32,
) -> Result<bool, AppErrorDto> {
    toggle(&state, track_id).map_err(AppErrorDto::from)
}

fn list(state: &AppState) -> AppResult<Vec<Track>> {
    let conn = state.conn.lock();
    let sql = format!(
        "{TRACK_SELECT} JOIN favorites f ON f.track_id = t.id \
         ORDER BY f.created_at DESC, f.rowid DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt.query_map([], row_to_track)?.collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub async fn favorites_list(state: State<'_, AppState>) -> Result<Vec<Track>, AppErrorDto> {
    list(&state).map_err(AppErrorDto::from)
}

fn all_ids(state: &AppState) -> AppResult<Vec<i32>> {
    let conn = state.conn.lock();
    let mut stmt = conn.prepare("SELECT track_id FROM favorites")?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    Ok(rows.map(|r| r.map(|v| v as i32)).collect::<Result<Vec<_>, _>>()?)
}

/// 收藏 id 集合（前端行内红心状态用，避免逐行查询）
#[tauri::command]
#[specta::specta]
pub async fn favorites_ids(state: State<'_, AppState>) -> Result<Vec<i32>, AppErrorDto> {
    all_ids(&state).map_err(AppErrorDto::from)
}
