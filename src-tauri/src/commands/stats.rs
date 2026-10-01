use rusqlite::{params, Connection};
use tauri::State;

use crate::commands::tracks::{row_to_track, TRACK_SELECT};
use crate::error::{AppErrorDto, AppResult};
use crate::models::Track;
use crate::AppState;

/// 最近播放：每首曲目取其最近一次播放时间倒序（功能设计文档 §3 统计）
fn recent(conn: &Connection, limit: u32) -> AppResult<Vec<Track>> {
    let sql = format!(
        "{TRACK_SELECT} \
         JOIN (SELECT track_id, MAX(played_at) AS last_played \
               FROM play_history GROUP BY track_id) h ON h.track_id = t.id \
         ORDER BY h.last_played DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params![i64::from(limit.clamp(1, 200))], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// 最常播放：按播放次数聚合（技术设计文档 §4：计数由聚合查询得出，不冗余存储）
fn top(conn: &Connection, limit: u32) -> AppResult<Vec<Track>> {
    let sql = format!(
        "{TRACK_SELECT} \
         JOIN (SELECT track_id, COUNT(*) AS play_count, MAX(played_at) AS last_played \
               FROM play_history GROUP BY track_id) h ON h.track_id = t.id \
         ORDER BY h.play_count DESC, h.last_played DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params![i64::from(limit.clamp(1, 200))], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub async fn history_recent(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<Track>, AppErrorDto> {
    recent(&state.conn.lock(), limit.unwrap_or(100)).map_err(AppErrorDto::from)
}

#[tauri::command]
#[specta::specta]
pub async fn history_top(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<Track>, AppErrorDto> {
    top(&state.conn.lock(), limit.unwrap_or(100)).map_err(AppErrorDto::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn seed_history(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO tracks(path, title, folder_id) VALUES
                 ('a.mp3', 'A', 1), ('b.mp3', 'B', 1), ('c.mp3', 'C', 1);
             INSERT INTO play_history(track_id, played_at) VALUES
                 (1, '2026-01-02 10:00:00'),
                 (2, '2026-01-01 10:00:00'),
                 (2, '2026-01-02 09:00:00'),
                 (2, '2026-01-03 09:00:00'),
                 (3, '2026-01-01 08:00:00');",
        )
        .unwrap();
    }

    #[test]
    fn 最近播放按最后播放时间倒序去重() {
        let conn = db::open_in_memory().unwrap();
        seed_history(&conn);
        let tracks = recent(&conn, 100).unwrap();
        let titles: Vec<&str> = tracks.iter().map(|t| t.title.as_str()).collect();
        // b 最后播于 01-03，a 01-02，c 01-01；每首曲目只出现一次
        assert_eq!(titles, vec!["B", "A", "C"]);
    }

    #[test]
    fn 最常播放按次数聚合并列时取最近() {
        let conn = db::open_in_memory().unwrap();
        seed_history(&conn);
        let tracks = top(&conn, 100).unwrap();
        let titles: Vec<&str> = tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["B", "A", "C"]);
    }

    #[test]
    fn 空历史返回空列表() {
        let conn = db::open_in_memory().unwrap();
        assert!(recent(&conn, 100).unwrap().is_empty());
        assert!(top(&conn, 100).unwrap().is_empty());
    }
}
