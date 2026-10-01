use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use rusqlite::{params, params_from_iter, Connection};
use walkdir::WalkDir;

use crate::error::AppResult;
use crate::library::metadata;

pub(crate) const AUDIO_EXTS: &[&str] = &["mp3", "flac", "m4a", "ogg", "oga", "opus", "wav"];
const PROGRESS_STEP: usize = 25;
const BATCH_SIZE: usize = 200;
const DELETE_CHUNK: usize = 500;

/// 扫描统计（headless 复现工具 scan_repro 直接打印）
#[derive(Debug, Clone, Copy)]
pub struct ScanOutcome {
    pub added: u32,
    pub updated: u32,
    pub removed: u32,
    pub unchanged: u32,
    pub errors: u32,
    pub duration_ms: u32,
}

/// 纯扫描流程，不依赖 tauri（bin/scan_repro.rs 复用）：
/// 收集各 folder 下音频文件 → 解析入库（folder_id 挂到所属目录）→ 删除已消失文件的记录
pub fn scan_folders(
    conn: &mut Connection,
    folders: &[(i64, String)],
    covers_dir: &Path,
    mut progress: impl FnMut(u32, u32),
) -> AppResult<ScanOutcome> {
    let started = Instant::now();

    // 已有曲目：path -> (id, mtime_ms, size)
    let mut existing: HashMap<String, (i64, i64, i64)> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT path, id, mtime_ms, size FROM tracks")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows {
            let (path, id, mtime, size) = row?;
            existing.insert(path, (id, mtime, size));
        }
    }

    // 阶段一：收集全部音频文件（所属 folder_id 一并记录；嵌套目录去重）
    let mut files: Vec<(String, i64)> = Vec::new();
    let mut collected: HashSet<String> = HashSet::new();
    for (folder_id, dir) in folders {
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let matched = entry
                .path()
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .map(|e| AUDIO_EXTS.contains(&e.as_str()))
                .unwrap_or(false);
            if matched {
                let path = entry.into_path().to_string_lossy().to_string();
                if collected.insert(path.clone()) {
                    files.push((path, *folder_id));
                }
            }
        }
    }
    let total = files.len() as u32;
    progress(0, total);

    // 阶段二：分批事务入库
    let (mut added, mut updated, mut unchanged, mut errors) = (0u32, 0u32, 0u32, 0u32);
    let mut seen: HashSet<String> = HashSet::with_capacity(files.len());
    let mut processed: usize = 0;

    for chunk in files.chunks(BATCH_SIZE) {
        let tx = conn.transaction()?;
        for (path, folder_id) in chunk {
            processed += 1;
            seen.insert(path.clone());

            let (mtime, size) = match std::fs::metadata(path) {
                Ok(md) => (
                    md.modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0),
                    md.len() as i64,
                ),
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[scan] metadata 读取失败 {path}: {e}");
                    errors += 1;
                    continue;
                }
            };

            if let Some(&(_, m, s)) = existing.get(path) {
                if m == mtime && s == size {
                    unchanged += 1;
                    continue;
                }
            }

            match metadata::parse(Path::new(path), covers_dir) {
                Ok(parsed) => {
                    let is_new = !existing.contains_key(path);
                    match upsert_track(&tx, path, *folder_id, &parsed, mtime, size) {
                        Ok(()) => {
                            if is_new {
                                added += 1;
                            } else {
                                updated += 1;
                            }
                        }
                        Err(e) => {
                            #[cfg(debug_assertions)]
                            eprintln!("[scan] 入库失败 {path}: {e}");
                            errors += 1;
                        }
                    }
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[scan] 解析失败 {path}: {e}");
                    errors += 1;
                }
            }

            if processed.is_multiple_of(PROGRESS_STEP) {
                progress(processed as u32, total);
            }
        }
        tx.commit()?;
    }

    // 阶段三：删除已消失的文件对应记录
    let removed_ids: Vec<i64> = existing
        .iter()
        .filter(|(path, _)| !seen.contains(*path))
        .map(|(_, (id, _, _))| *id)
        .collect();
    let removed = removed_ids.len() as u32;
    for chunk in removed_ids.chunks(DELETE_CHUNK) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM tracks WHERE id IN ({placeholders})");
        conn.execute(&sql, params_from_iter(chunk.iter()))?;
    }

    Ok(ScanOutcome {
        added,
        updated,
        removed,
        unchanged,
        errors,
        duration_ms: started.elapsed().as_millis() as u32,
    })
}

fn upsert_track(
    conn: &rusqlite::Connection,
    path: &str,
    folder_id: i64,
    p: &metadata::ParsedTrack,
    mtime: i64,
    size: i64,
) -> AppResult<()> {
    let artist_id = p
        .artist
        .as_deref()
        .map(|name| get_or_create_artist(conn, name))
        .transpose()?;
    let album_id = match p.album.as_deref() {
        Some(album) => {
            let album_artist = p.artist.as_deref().unwrap_or("");
            Some(get_or_create_album(conn, album, album_artist, p.year, p.cover_file.as_deref())?)
        }
        None => None,
    };

    conn.execute(
        "INSERT INTO tracks (path, folder_id, title, artist_id, album_id, track_no, disc,
                             duration_sec, sample_rate, bitrate, year, genre, cover_file,
                             lyrics, mtime_ms, size)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
         ON CONFLICT(path) DO UPDATE SET
            folder_id = excluded.folder_id,
            title = excluded.title,
            artist_id = excluded.artist_id,
            album_id = excluded.album_id,
            track_no = excluded.track_no,
            disc = excluded.disc,
            duration_sec = excluded.duration_sec,
            sample_rate = excluded.sample_rate,
            bitrate = excluded.bitrate,
            year = excluded.year,
            genre = excluded.genre,
            cover_file = excluded.cover_file,
            lyrics = excluded.lyrics,
            mtime_ms = excluded.mtime_ms,
            size = excluded.size,
            updated_at = datetime('now')",
        params![
            path,
            folder_id,
            p.title,
            artist_id,
            album_id,
            p.track_no,
            p.disc,
            p.duration_sec,
            p.sample_rate,
            p.bitrate,
            p.year,
            p.genre,
            p.cover_file,
            p.lyrics,
            mtime,
            size
        ],
    )?;
    Ok(())
}

fn get_or_create_artist(conn: &rusqlite::Connection, name: &str) -> AppResult<i64> {
    conn.execute("INSERT OR IGNORE INTO artists(name) VALUES (?1)", params![name])?;
    Ok(conn.query_row("SELECT id FROM artists WHERE name = ?1", params![name], |r| r.get(0))?)
}

fn get_or_create_album(
    conn: &rusqlite::Connection,
    name: &str,
    artist: &str,
    year: Option<i64>,
    cover_file: Option<&str>,
) -> AppResult<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO albums(name, artist, year, cover_file) VALUES (?1, ?2, ?3, ?4)",
        params![name, artist, year, cover_file],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM albums WHERE name = ?1 AND artist = ?2",
        params![name, artist],
        |r| r.get(0),
    )?;
    if let Some(cover) = cover_file {
        conn.execute(
            "UPDATE albums SET cover_file = ?1 WHERE id = ?2 AND (cover_file IS NULL OR cover_file = '')",
            params![cover, id],
        )?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn 空目录扫描产出全零统计() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../../migrations/0001_init.sql")).unwrap();
        let covers = std::env::temp_dir().join("tinymusic-test-covers");
        std::fs::create_dir_all(&covers).unwrap();
        let outcome = scan_folders(&mut conn, &[(1, "Z:/不存在/目录".into())], &covers, |_, _| {})
            .unwrap();
        assert_eq!((outcome.added, outcome.errors, outcome.removed), (0, 0, 0));
    }

    #[test]
    fn 迁移后tracks表含folder_id列() {
        let conn = db::open_in_memory().unwrap();
        let cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(tracks)").unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(1)).unwrap();
            rows.collect::<Result<Vec<_>, _>>().unwrap()
        };
        assert!(cols.iter().any(|c| c == "folder_id"), "tracks 必须有 folder_id 列");
    }
}
