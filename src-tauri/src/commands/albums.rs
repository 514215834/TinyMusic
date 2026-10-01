use rusqlite::params;
use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::models::{AlbumInfo, ArtistInfo, Track};
use crate::AppState;

use super::tracks::{order_clause, row_to_track, TRACK_SELECT};

fn query_albums(state: &AppState) -> AppResult<Vec<AlbumInfo>> {
    let conn = state.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT al.id, al.name, al.artist, al.year, al.cover_file, COUNT(t.id) \
         FROM albums al JOIN tracks t ON t.album_id = al.id \
         GROUP BY al.id \
         ORDER BY al.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AlbumInfo {
            id: r.get::<_, i64>(0)? as i32,
            name: r.get(1)?,
            artist: r.get(2)?,
            year: r.get::<_, Option<i64>>(3)?.map(|v| v as i32),
            cover_file: r.get(4)?,
            track_count: r.get::<_, i64>(5)? as i32,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
#[specta::specta]
pub async fn albums_query(state: State<'_, AppState>) -> Result<Vec<AlbumInfo>, AppErrorDto> {
    query_albums(&state).map_err(AppErrorDto::from)
}

fn query_artists(state: &AppState) -> AppResult<Vec<ArtistInfo>> {
    let conn = state.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT ar.id, ar.name, COUNT(t.id), COUNT(DISTINCT t.album_id) \
         FROM artists ar JOIN tracks t ON t.artist_id = ar.id \
         GROUP BY ar.id \
         ORDER BY ar.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ArtistInfo {
            id: r.get::<_, i64>(0)? as i32,
            name: r.get(1)?,
            track_count: r.get::<_, i64>(2)? as i32,
            album_count: r.get::<_, i64>(3)? as i32,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
#[specta::specta]
pub async fn artists_query(state: State<'_, AppState>) -> Result<Vec<ArtistInfo>, AppErrorDto> {
    query_artists(&state).map_err(AppErrorDto::from)
}

/// 专辑内曲目按 碟号/曲号/标题 排序
fn query_album_tracks(state: &AppState, album_id: i32) -> AppResult<Vec<Track>> {
    let conn = state.conn.lock();
    let sql = format!(
        "{TRACK_SELECT} WHERE t.album_id = ?1 \
         ORDER BY t.disc, t.track_no, t.title COLLATE NOCASE"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params![i64::from(album_id)], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub async fn album_tracks(
    state: State<'_, AppState>,
    album_id: i32,
) -> Result<Vec<Track>, AppErrorDto> {
    query_album_tracks(&state, album_id).map_err(AppErrorDto::from)
}

fn query_artist_tracks(state: &AppState, artist_id: i32) -> AppResult<Vec<Track>> {
    let conn = state.conn.lock();
    let sql = format!(
        "{TRACK_SELECT} WHERE t.artist_id = ?1 {}",
        order_clause(None)
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params![i64::from(artist_id)], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub async fn artist_tracks(
    state: State<'_, AppState>,
    artist_id: i32,
) -> Result<Vec<Track>, AppErrorDto> {
    query_artist_tracks(&state, artist_id).map_err(AppErrorDto::from)
}
