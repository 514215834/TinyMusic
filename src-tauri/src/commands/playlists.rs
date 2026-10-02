use rusqlite::{params, Connection};
use tauri::State;

use crate::error::{AppError, AppErrorDto, AppResult};
use crate::library::m3u8;
use crate::models::{Playlist, PlaylistImport, Track};
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
         FROM playlists p ORDER BY p.position, p.id",
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

/// 侧边栏歌单拖拽重排（M7+）：按传入顺序写 position（全量提交，单事务）
fn reorder_playlists(state: &AppState, ids: &[i32]) -> AppResult<()> {
    let mut conn = state.conn.lock();
    let tx = conn.transaction()?;
    for (idx, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE playlists SET position = ?1 WHERE id = ?2",
            params![idx as i64, i64::from(*id)],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_reorder_playlists(
    state: State<'_, AppState>,
    ids: Vec<i32>,
) -> Result<(), AppErrorDto> {
    reorder_playlists(&state, &ids).map_err(AppErrorDto::from)
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_delete(state: State<'_, AppState>, id: i32) -> Result<(), AppErrorDto> {
    delete(&state, id).map_err(AppErrorDto::from)
}

fn tracks(conn: &Connection, id: i32) -> AppResult<Vec<Track>> {
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
    tracks(&state.conn.lock(), id).map_err(AppErrorDto::from)
}

/// 导出歌单为 UTF-8 M3U8 文件（绝对路径，foobar2000 兼容），返回写入曲目数
#[tauri::command]
#[specta::specta]
pub async fn playlist_export(
    state: State<'_, AppState>,
    id: i32,
    path: String,
) -> Result<u32, AppErrorDto> {
    let track_list = tracks(&state.conn.lock(), id).map_err(AppErrorDto::from)?;
    let count = track_list.len() as u32;
    let text = m3u8::format_m3u8(&track_list);
    std::fs::write(&path, text).map_err(AppError::from)?;
    Ok(count)
}

/// M3U8 条目 → 新歌单：按曲库路径归一化匹配，未命中曲目跳过不计入
fn import_entries(
    conn: &mut Connection,
    entries: &[m3u8::M3u8Entry],
    name: &str,
) -> AppResult<PlaylistImport> {
    let name = clean_name(name)?;
    let path_to_id: std::collections::HashMap<String, i64> = {
        let mut stmt = conn.prepare("SELECT id, path FROM tracks")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<Result<std::collections::HashMap<_, _>, _>>()?
            .into_iter()
            .map(|(id, path)| (m3u8::normalize_path(&path), id))
            .collect()
    };

    let matched: Vec<i64> = entries
        .iter()
        .filter_map(|e| path_to_id.get(&m3u8::normalize_path(&e.path)).copied())
        .collect();
    if matched.is_empty() {
        return Err(AppError::Message(
            "M3U8 中的曲目都不在当前曲库：请先导入对应音乐文件夹".into(),
        ));
    }
    let skipped = entries.len() - matched.len();

    let tx = conn.transaction()?;
    tx.execute("INSERT INTO playlists(name) VALUES (?1)", params![name])?;
    let playlist_id: i64 =
        tx.query_row("SELECT last_insert_rowid()", [], |r| r.get(0))?;
    for (pos, track_id) in matched.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position) \
             VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, pos as i64],
        )?;
    }
    tx.commit()?;
    Ok(PlaylistImport {
        playlist_id: playlist_id as i32,
        added: matched.len() as u32,
        skipped: skipped as u32,
    })
}

/// 从 M3U8 文件导入为新歌单，返回 新歌单id/导入数/跳过数
#[tauri::command]
#[specta::specta]
pub async fn playlist_import(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<PlaylistImport, AppErrorDto> {
    let bytes = std::fs::read(&path).map_err(AppError::from)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| AppError::Message("M3U8 文件必须是 UTF-8 编码".into()))?;
    let entries = m3u8::parse_m3u8(&text);
    if entries.is_empty() {
        return Err(AppError::Message("M3U8 中没有曲目".into()).into());
    }
    let mut conn = state.conn.lock();
    import_entries(&mut conn, &entries, &name).map_err(AppErrorDto::from)
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
    use super::import_entries;
    use crate::db;
    use crate::library::m3u8;
    use rusqlite::params;

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

    #[test]
    fn m3u8导入按归一化路径匹配并跳过未命中() {
        let mut conn = db::open_in_memory().unwrap();
        seed(&conn);
        let entries = vec![
            m3u8::M3u8Entry {
                path: "A.MP3".into(), // 大小写差异命中
                title: None,
                duration_sec: None,
            },
            m3u8::M3u8Entry {
                path: "C:/曲库/b.MP3".into(), // 路径不同：不命中
                title: None,
                duration_sec: None,
            },
            m3u8::M3u8Entry {
                path: "b.mp3".into(),
                title: None,
                duration_sec: None,
            },
            m3u8::M3u8Entry {
                path: "missing.mp3".into(),
                title: None,
                duration_sec: None,
            },
        ];
        let result = import_entries(&mut conn, &entries, "导入歌单").unwrap();
        assert_eq!(result.added, 2);
        assert_eq!(result.skipped, 2);
        // 顺序保持 M3U8 原始顺序（a 在 b 前）
        let titles: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT t.title FROM tracks t \
                     JOIN playlist_tracks pt ON pt.track_id = t.id \
                     WHERE pt.playlist_id = ?1 ORDER BY pt.position",
                )
                .unwrap();
            stmt.query_map(params![result.playlist_id], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(titles, vec!["A", "B"]);
    }

    #[test]
    fn m3u8导入全部未命中时报错() {
        let mut conn = db::open_in_memory().unwrap();
        seed(&conn);
        let entries = vec![m3u8::M3u8Entry {
            path: "nope.mp3".into(),
            title: None,
            duration_sec: None,
        }];
        assert!(import_entries(&mut conn, &entries, "X").is_err());
    }

    #[test]
    fn m3u8导入拒绝空白歌单名() {
        let mut conn = db::open_in_memory().unwrap();
        seed(&conn);
        let entries = vec![m3u8::M3u8Entry {
            path: "a.mp3".into(),
            title: None,
            duration_sec: None,
        }];
        assert!(import_entries(&mut conn, &entries, "  ").is_err());
    }
}
