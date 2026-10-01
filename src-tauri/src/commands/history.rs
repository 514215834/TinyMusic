use rusqlite::params;
use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::AppState;

/// 播放入库（最近/最常播放数据源）。前端在曲目变化时调用一次即可（客户端节流）
fn add(state: &AppState, track_id: i32) -> AppResult<()> {
    let conn = state.conn.lock();
    conn.execute(
        "INSERT INTO play_history(track_id) VALUES (?1)",
        params![i64::from(track_id)],
    )?;
    // 历史无限增长没有意义：仅保留最近 5000 条
    conn.execute(
        "DELETE FROM play_history WHERE id NOT IN (\
            SELECT id FROM play_history ORDER BY id DESC LIMIT 5000)",
        [],
    )?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn history_add(state: State<'_, AppState>, track_id: i32) -> Result<(), AppErrorDto> {
    add(&state, track_id).map_err(AppErrorDto::from)
}
