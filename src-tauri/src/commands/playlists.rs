use rusqlite::params;
use tauri::State;

use crate::error::{AppError, AppErrorDto, AppResult};
use crate::models::{Playlist, Track};
use crate::AppState;

use super::tracks::{row_to_track, TRACK_SELECT};

fn playlist_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Playlist> {
    Ok(Playlist {
        id: r.get::<_, i64>(0)? as i32,
        name: r.get(1)?,
        track_count: r.get::<_, i64>(2)? as i32,
        created_at: r.get(3)?,
    })
}

/// 归一化歌单名：去首尾空白；空名拒绝
fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Message("歌单名不能为空".into()));
    }
    Ok(name.chars().take(100).collect())
}

fn list(state: &AppState) -> AppResult<Vec<Playlist>> {
    let conn = state.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, \
            (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id), \
            p.created_at \
         FROM playlists p ORDER BY p.id",
    )?;
    let rows = stmt.query_map([], playlist_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_list(state: State<'_, AppState>) -> Result<Vec<Playlist>, AppErrorDto> {
    list(&state).map_err(AppErrorDto::from)
}

fn create(state: &AppState, name: &str) -> AppResult<Playlist> {
    let name = clean_name(name)?;
    let mut conn = state.conn.lock();
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO playlists(name) VALUES (?1)", params![name])?;
    let id: i64 = tx.query_row("SELECT id FROM playlists WHERE name = ?1", params![name], |r| r.get(0))?;
    let playlist = Playlist { id: id as i32, name, track_count: 0, created_at: String::new() };
    tx.commit()?;
    Ok(playlist)
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_create(
    state: State<'_, AppState>,
    name: String,
) -> Result<Playlist, AppErrorDto> {
    create(&state, &name).map_err(AppErrorDto::from)
}

fn rename(state: &AppState, id: i32, name: &str) -> AppResult<()> {
    let name = clean_name(name)?;
    let conn = state.conn.lock();
    let n = conn.execute(
        "UPDATE playlists SET name = ?1 WHERE id = ?2",
        params![name, i64::from(id)],
    )?;
    if n == 0 {
        return Err(AppError::Message("歌单不存在".into()));
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_rename(
    state: State<'_, AppState>,
    id: i32,
    name: String,
) -> Result<(), AppErrorDto> {
    rename(&state, id, &name).map_err(AppErrorDto::from)
}

fn delete(state: &AppState, id: i32) -> AppResult<()> {
    let conn = state.conn.lock();
    conn.execute("DELETE FROM playlists WHERE id = ?1", params![i64::from(id)])?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_delete(state: State<'_, AppState>, id: i32) -> Result<(), AppErrorDto> {
    delete(&state, id).map_err(AppErrorDto::from)
}

fn tracks(state: &AppState, id: i32) -> AppResult<Vec<Track>> {
    let conn = state.conn.lock();
    let sql = format!(
        "{TRACK_SELECT} JOIN playlist_tracks pt ON pt.track_id = t.id \
         WHERE pt.playlist_id = ?1 ORDER BY pt.position"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params![i64::from(id)], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_tracks(
    state: State<'_, AppState>,
    id: i32,
) -> Result<Vec<Track>, AppErrorDto> {
    tracks(&state, id).map_err(AppErrorDto::from)
}

/// 追加曲目（已存在的自动跳过），返回歌单曲目数
fn add_tracks(state: &AppState, id: i32, track_ids: &[i32]) -> AppResult<i32> {
    let mut conn = state.conn.lock();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO playlist_tracks(playlist_id, track_id, position) \
         SELECT ?1, t.id, \
                COALESCE((SELECT MAX(position) FROM playlist_tracks WHERE playlist_id = ?1), -1) + 1 \
         FROM tracks t WHERE t.id IN (SELECT value FROM json_each(?2)) \
         ON CONFLICT(playlist_id, track_id) DO NOTHING",
        params![i64::from(id), serde_json::to_string(track_ids)?],
    )?;
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1",
        params![i64::from(id)],
        |r| r.get(0),
    )?;
    tx.commit()?;
    Ok(count as i32)
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_add_tracks(
    state: State<'_, AppState>,
    id: i32,
    track_ids: Vec<i32>,
) -> Result<i32, AppErrorDto> {
    add_tracks(&state, id, &track_ids).map_err(AppErrorDto::from)
}

/// 移除单曲并把剩余曲目位置压实为 0..n，返回剩余曲目数
fn remove_track(state: &AppState, id: i32, track_id: i32) -> AppResult<i32> {
    let mut conn = state.conn.lock();
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
        params![i64::from(id), i64::from(track_id)],
    )?;
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1",
        params![i64::from(id)],
        |r| r.get(0),
    )?;
    tx.execute(
        "UPDATE playlist_tracks SET position = (\
            SELECT COUNT(*) FROM playlist_tracks p2 \
            WHERE p2.playlist_id = playlist_tracks.playlist_id \
              AND p2.position < playlist_tracks.position) \
         WHERE playlist_id = ?1",
        params![i64::from(id)],
    )?;
    tx.commit()?;
    Ok(count as i32)
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_remove_track(
    state: State<'_, AppState>,
    id: i32,
    track_id: i32,
) -> Result<i32, AppErrorDto> {
    remove_track(&state, id, track_id).map_err(AppErrorDto::from)
}

/// 全量重排（前端拖拽后按新顺序传全部 track_id），返回歌单曲目数
fn reorder(state: &AppState, id: i32, track_ids: &[i32]) -> AppResult<i32> {
    let mut conn = state.conn.lock();
    let tx = conn.transaction()?;
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1",
        params![i64::from(id)],
        |r| r.get(0),
    )?;
    if track_ids.len() != count as usize {
        return Err(AppError::Message("重排数据与歌单内容不一致".into()));
    }
    for (pos, track_id) in track_ids.iter().enumerate() {
        tx.execute(
            "UPDATE playlist_tracks SET position = ?1 \
             WHERE playlist_id = ?2 AND track_id = ?3",
            params![pos as i64, i64::from(id), i64::from(*track_id)],
        )?;
    }
    tx.commit()?;
    Ok(count as i32)
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_reorder(
    state: State<'_, AppState>,
    id: i32,
    track_ids: Vec<i32>,
) -> Result<i32, AppErrorDto> {
    reorder(&state, id, &track_ids).map_err(AppErrorDto::from)
}

#[cfg(test)]
mod tests {
    use crate::db;

    fn seed(conn: &rusqlite::Connection) -> (i64, Vec<i64>) {
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO tracks(path, title, folder_id) VALUES
               ('a.mp3', 'A', 1), ('b.mp3', 'B', 1), ('c.mp3', 'C', 1);",
        )
        .unwrap();
        let pid: i64 = conn
            .query_row("INSERT INTO playlists(name) VALUES ('测试') RETURNING id", [], |r| r.get(0))
            .unwrap();
        let ids: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM tracks ORDER BY id").unwrap();
            stmt.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
        };
        (pid, ids)
    }

    #[test]
    fn 添加去重与位置压实() {
        let conn = db::open_in_memory().unwrap();
        let (pid, ids) = seed(&conn);
        for (i, id) in ids.iter().enumerate() {
            conn.execute(
                "INSERT INTO playlist_tracks(playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                rusqlite::params![pid, id, i as i64],
            )
            .unwrap();
        }
        // 重复添加被主键忽略
        let n = conn
            .execute(
                "INSERT INTO playlist_tracks(playlist_id, track_id, position) \
                 SELECT ?1, t.id, 99 FROM tracks t WHERE t.id = ?2 \
                 ON CONFLICT(playlist_id, track_id) DO NOTHING",
                rusqlite::params![pid, ids[0]],
            )
            .unwrap();
        assert_eq!(n, 0);

        // 删除中间曲目后位置压实
        conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            rusqlite::params![pid, ids[1]],
        )
        .unwrap();
        conn.execute(
            "UPDATE playlist_tracks SET position = (\
                SELECT COUNT(*) FROM playlist_tracks p2 \
                WHERE p2.playlist_id = playlist_tracks.playlist_id \
                  AND p2.position < playlist_tracks.position) \
             WHERE playlist_id = ?1",
            rusqlite::params![pid],
        )
        .unwrap();
        let positions: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT position FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")
                .unwrap();
            stmt.query_map(rusqlite::params![pid], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(positions, vec![0, 1]);
    }
}
