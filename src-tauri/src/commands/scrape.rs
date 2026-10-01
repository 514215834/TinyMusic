//! 在线刮削命令（M4）：手动触发，返回候选由前端确认后应用（歧义人工确认）。
//! 命中落库：封面入 covers/ 缓存、专辑信息更新 albums/tracks、歌词复用 tracks.lyrics 与 .lrc 通道。
//! DB 读取在 await 之前完成，parking_lot 锁不跨 await。

use std::path::Path;

use rusqlite::params;
use tauri::State;

use crate::error::{AppError, AppErrorDto, AppResult};
use crate::library::{covers, scrape};
use crate::models::{AlbumCandidate, LyricsCandidate, ScrapeSource};
use crate::AppState;

/// 检索词：自定义 query 优先（用户在刮削弹窗可改写检索信息），空则回落到标签元数据
fn effective_term(query: Option<&str>, fallback: String) -> String {
    match query.map(str::trim).filter(|s| !s.is_empty()) {
        Some(q) => q.to_string(),
        None => fallback,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn scrape_album(
    state: State<'_, AppState>,
    album_id: i32,
    source: Option<ScrapeSource>,
    query: Option<String>,
) -> Result<Vec<AlbumCandidate>, AppErrorDto> {
    let (name, artist): (String, String) = {
        let conn = state.conn.lock();
        conn.query_row(
            "SELECT name, artist FROM albums WHERE id = ?1",
            params![i64::from(album_id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(AppError::from)?
    };
    let term = effective_term(
        query.as_deref(),
        format!("{} {}", name.trim(), artist.trim()).trim().to_string(),
    );
    if term.is_empty() {
        return Err(AppError::Message("检索词为空，无法刮削".into()).into());
    }
    scrape::search_candidates(&term, "album", source)
        .await
        .map_err(AppErrorDto::from)
}

#[tauri::command]
#[specta::specta]
pub async fn scrape_track(
    state: State<'_, AppState>,
    track_id: i32,
    source: Option<ScrapeSource>,
    query: Option<String>,
) -> Result<Vec<AlbumCandidate>, AppErrorDto> {
    let (title, artist): (String, Option<String>) = {
        let conn = state.conn.lock();
        conn.query_row(
            "SELECT t.title, ar.name FROM tracks t LEFT JOIN artists ar ON ar.id = t.artist_id \
             WHERE t.id = ?1",
            params![i64::from(track_id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(AppError::from)?
    };
    let term = effective_term(
        query.as_deref(),
        format!("{} {}", title.trim(), artist.as_deref().unwrap_or("").trim())
            .trim()
            .to_string(),
    );
    if term.is_empty() {
        return Err(AppError::Message("检索词为空，无法刮削".into()).into());
    }
    scrape::search_candidates(&term, "song", source)
        .await
        .map_err(AppErrorDto::from)
}

/// 下载封面入 covers/ 缓存，返回缓存文件名
async fn fetch_cover(artwork_url: &str, covers_dir: &Path) -> AppResult<String> {
    let (data, ext) = scrape::download_image(artwork_url).await?;
    covers::save(covers_dir, &data, ext).map_err(AppError::from)
}

/// 应用专辑候选：封面入缓存 → albums.cover_file/year 更新 → 专辑内曲目的 year/genre 同步
#[tauri::command]
#[specta::specta]
pub async fn scrape_apply_album(
    state: State<'_, AppState>,
    album_id: i32,
    candidate: AlbumCandidate,
) -> Result<Option<String>, AppErrorDto> {
    let covers_dir = state.covers_dir.clone();
    let cover_file = match &candidate.artwork_url {
        Some(url) => Some(fetch_cover(url, &covers_dir).await?),
        None => None,
    };
    if cover_file.is_none() && candidate.year.is_none() && candidate.genre.is_none() {
        return Err(AppError::Message("该候选没有可应用的封面或专辑信息".into()).into());
    }
    let conn = state.conn.lock();
    if let Some(cover) = &cover_file {
        conn.execute(
            "UPDATE albums SET cover_file = ?1 WHERE id = ?2",
            params![cover, i64::from(album_id)],
        )
        .map_err(AppError::from)?;
    }
    if let Some(year) = candidate.year {
        conn.execute("UPDATE albums SET year = ?1 WHERE id = ?2", params![year, i64::from(album_id)])
            .map_err(AppError::from)?;
        conn.execute(
            "UPDATE tracks SET year = ?1 WHERE album_id = ?2",
            params![year, i64::from(album_id)],
        )
        .map_err(AppError::from)?;
    }
    if let Some(genre) = &candidate.genre {
        conn.execute(
            "UPDATE tracks SET genre = ?1 WHERE album_id = ?2",
            params![genre, i64::from(album_id)],
        )
        .map_err(AppError::from)?;
    }
    Ok(cover_file)
}

/// 应用单曲候选：仅该曲目的封面（文本字段走标签编辑，职责分离）
#[tauri::command]
#[specta::specta]
pub async fn scrape_apply_track(
    state: State<'_, AppState>,
    track_id: i32,
    candidate: AlbumCandidate,
) -> Result<String, AppErrorDto> {
    let cover_file = match &candidate.artwork_url {
        Some(url) => {
            let covers_dir = state.covers_dir.clone();
            fetch_cover(url, &covers_dir).await?
        }
        None => return Err(AppError::Message("该候选没有封面".into()).into()),
    };
    let conn = state.conn.lock();
    conn.execute(
        "UPDATE tracks SET cover_file = ?1 WHERE id = ?2",
        params![cover_file, i64::from(track_id)],
    )
    .map_err(AppError::from)?;
    Ok(cover_file)
}

#[tauri::command]
#[specta::specta]
pub async fn scrape_lyrics(
    state: State<'_, AppState>,
    track_id: i32,
    query: Option<String>,
) -> Result<Vec<LyricsCandidate>, AppErrorDto> {
    let (title, artist, album, duration): (String, Option<String>, Option<String>, Option<f64>) = {
        let conn = state.conn.lock();
        conn.query_row(
            "SELECT t.title, ar.name, al.name, t.duration_sec FROM tracks t \
             LEFT JOIN artists ar ON ar.id = t.artist_id \
             LEFT JOIN albums al ON al.id = t.album_id WHERE t.id = ?1",
            params![i64::from(track_id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(AppError::from)?
    };
    // 自定义检索词走 LRCLIB q 模糊检索；否则按 标签 精确字段检索
    if let Some(q) = query.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return scrape::search_lyrics_q(q, duration)
            .await
            .map_err(AppErrorDto::from);
    }
    scrape::search_lyrics(
        title.trim(),
        artist.as_deref().unwrap_or("").trim(),
        album.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        duration,
    )
    .await
    .map_err(AppErrorDto::from)
}

/// 应用歌词候选：同步歌词优先；同名 .lrc 已存在时一并更新，保持两条通道一致
#[tauri::command]
#[specta::specta]
pub async fn scrape_apply_lyrics(
    state: State<'_, AppState>,
    track_id: i32,
    candidate: LyricsCandidate,
) -> Result<(), AppErrorDto> {
    let text = candidate
        .synced_lyrics
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| candidate.plain_lyrics.clone().filter(|s| !s.trim().is_empty()));
    let text = match text {
        Some(t) => t,
        None if candidate.instrumental => {
            return Err(AppError::Message("该曲目在源站标记为纯音乐（无歌词）".into()).into())
        }
        None => return Err(AppError::Message("该候选没有歌词内容".into()).into()),
    };

    let path: String = {
        let conn = state.conn.lock();
        conn.query_row(
            "SELECT path FROM tracks WHERE id = ?1",
            params![i64::from(track_id)],
            |r| r.get(0),
        )
        .map_err(AppError::from)?
    };

    // .lrc 已存在才同步更新（不主动在音乐目录创建新文件）
    let lrc_path = Path::new(&path).with_extension("lrc");
    if lrc_path.is_file() {
        if let Some(synced) = &candidate.synced_lyrics {
            std::fs::write(&lrc_path, synced).map_err(AppError::from)?;
        }
    }

    let conn = state.conn.lock();
    conn.execute(
        "UPDATE tracks SET lyrics = ?1 WHERE id = ?2",
        params![text, i64::from(track_id)],
    )
    .map_err(AppError::from)?;
    Ok(())
}
