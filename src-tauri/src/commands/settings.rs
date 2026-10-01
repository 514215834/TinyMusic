use rusqlite::params;
use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::AppState;

fn get_setting(state: &AppState, key: &str) -> AppResult<Option<String>> {
    let conn = state.conn.lock();
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query_map(params![key], |r| r.get(0))?;
    Ok(rows.next().transpose()?)
}

fn set_setting(state: &AppState, key: &str, value: &str) -> AppResult<()> {
    let conn = state.conn.lock();
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn settings_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, AppErrorDto> {
    get_setting(&state, &key).map_err(AppErrorDto::from)
}

#[tauri::command]
#[specta::specta]
pub async fn settings_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppErrorDto> {
    set_setting(&state, &key, &value).map_err(AppErrorDto::from)
}
