use rusqlite::params;
use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::models::{Track, TrackPage};
use crate::AppState;

pub(crate) const TRACK_SELECT: &str = "SELECT t.id, t.path, t.title, ar.name, al.name, t.track_no, \
     t.duration_sec, t.sample_rate, t.bitrate, t.year, t.genre, t.cover_file \
     FROM tracks t \
     LEFT JOIN artists ar ON ar.id = t.artist_id \
     LEFT JOIN albums al ON al.id = t.album_id";

const SORTS: &[(&str, &str)] = &[
    ("title", "t.title COLLATE NOCASE"),
    ("artist", "ar.name COLLATE NOCASE"),
    ("album", "al.name COLLATE NOCASE"),
    ("duration", "t.duration_sec"),
    ("year", "t.year"),
];

/// 排序白名单：默认按 歌手/专辑/碟号/曲号/标题（技术设计文档 §5）
pub(crate) fn order_clause(sort: Option<&str>) -> String {
    let key = sort.unwrap_or("");
    let column = SORTS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, c)| *c)
        .unwrap_or("ar.name COLLATE NOCASE, al.name COLLATE NOCASE, t.disc, t.track_no, t.title COLLATE NOCASE");
    format!("ORDER BY {column}")
}

/// DB 层 i64 → DTO i32（specta 禁止导出 i64）
pub(crate) fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get::<_, i64>(0)? as i32,
        path: row.get(1)?,
        title: row.get(2)?,
        artist: row.get(3)?,
        album: row.get(4)?,
        track_no: row.get::<_, Option<i64>>(5)?.map(|v| v as i32),
        duration_sec: row.get(6)?,
        sample_rate: row.get::<_, Option<i64>>(7)?.map(|v| v as i32),
        bitrate: row.get::<_, Option<i64>>(8)?.map(|v| v as i32),
        year: row.get::<_, Option<i64>>(9)?.map(|v| v as i32),
        genre: row.get(10)?,
        cover_file: row.get(11)?,
    })
}

fn query_tracks(
    state: &AppState,
    page: Option<u32>,
    page_size: Option<u32>,
    sort: Option<String>,
) -> AppResult<TrackPage> {
    let page = page.unwrap_or(1).max(1) as i64;
    let size = page_size.unwrap_or(200).clamp(1, 1000) as i64;
    let offset = (page - 1) * size;

    let conn = state.conn.lock();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
    let sql = format!("{TRACK_SELECT} {} LIMIT ?1 OFFSET ?2", order_clause(sort.as_deref()));
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt.query_map(params![size, offset], row_to_track)?.collect::<Result<Vec<_>, _>>()?;
    Ok(TrackPage { items, total: total as i32 })
}

#[tauri::command]
#[specta::specta]
pub async fn tracks_query(
    state: State<'_, AppState>,
    page: Option<u32>,
    page_size: Option<u32>,
    sort: Option<String>,
) -> Result<TrackPage, AppErrorDto> {
    query_tracks(&state, page, page_size, sort).map_err(AppErrorDto::from)
}

fn get_track(state: &AppState, id: i32) -> AppResult<Option<Track>> {
    let conn = state.conn.lock();
    let sql = format!("{TRACK_SELECT} WHERE t.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map(params![i64::from(id)], row_to_track)?;
    Ok(rows.next().transpose()?)
}

#[tauri::command]
#[specta::specta]
pub async fn track_get(state: State<'_, AppState>, id: i32) -> Result<Option<Track>, AppErrorDto> {
    get_track(&state, id).map_err(AppErrorDto::from)
}
