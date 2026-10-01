use std::path::Path;

use lofty::prelude::*;
use lofty::tag::ItemKey;
use rusqlite::params;
use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::AppState;

/// 取歌词：同名 .lrc 优先（最新内容）→ DB 内嵌歌词缓存 →
/// 现场解析一次并缓存（老曲目入库时未提取歌词的自愈路径）
fn get(state: &AppState, track_id: i32) -> AppResult<Option<String>> {
    let (path, cached): (String, Option<String>) = {
        let conn = state.conn.lock();
        let row = conn.query_row(
            "SELECT path, lyrics FROM tracks WHERE id = ?1",
            params![i64::from(track_id)],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        );
        match row {
            Ok(v) => v,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        }
    };

    let lrc_path = Path::new(&path).with_extension("lrc");
    if lrc_path.is_file() {
        if let Ok(s) = std::fs::read_to_string(&lrc_path) {
            if !s.trim().is_empty() {
                return Ok(Some(s));
            }
        }
    }
    if cached.as_deref().is_some_and(|s| !s.trim().is_empty()) {
        return Ok(cached);
    }

    let lyrics = parse_embedded(Path::new(&path));
    if let Some(s) = &lyrics {
        let conn = state.conn.lock();
        conn.execute(
            "UPDATE tracks SET lyrics = ?1 WHERE id = ?2",
            params![s, i64::from(track_id)],
        )?;
    }
    Ok(lyrics)
}

fn parse_embedded(path: &Path) -> Option<String> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let lyrics = tag.get_string(ItemKey::Lyrics)?.trim();
    (!lyrics.is_empty()).then(|| lyrics.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn lyrics_get(
    state: State<'_, AppState>,
    track_id: i32,
) -> Result<Option<String>, AppErrorDto> {
    get(&state, track_id).map_err(AppErrorDto::from)
}
