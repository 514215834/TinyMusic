//! 标签编辑（M4）：lofty 写回源文件（评审确认接受写回），曲库同步；
//! mtime/size 同步更新后，增量扫描按 unchanged 跳过（技术设计文档 §5 tag_update）。

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::AudioFile;
use lofty::prelude::*;
use lofty::tag::items::Timestamp;
use lofty::tag::Tag;
use rusqlite::params;
use tauri::State;

use crate::error::{AppError, AppErrorDto, AppResult};
use crate::models::{TagPatch, Track};
use crate::AppState;

use super::tracks::{row_to_track, TRACK_SELECT};

/// 文本字段：Some(非空) = 写入，Some(空白)/None = 清除
fn set_or_remove(
    tag: &mut Tag,
    value: Option<&str>,
    set: impl FnOnce(&mut Tag, String),
    remove: impl FnOnce(&mut Tag),
) {
    match value {
        Some(s) if !s.trim().is_empty() => set(tag, s.trim().to_string()),
        _ => remove(tag),
    }
}

/// 把标签写回音频源文件（无标签的文件先创建主标签区）
pub(crate) fn write_tags(path: &Path, patch: &TagPatch) -> AppResult<()> {
    let meta = std::fs::metadata(path)?;
    if meta.permissions().readonly() {
        return Err(AppError::Message("文件为只读，无法写回标签".into()));
    }

    let mut tagged = lofty::read_from_path(path)?;
    if tagged.primary_tag().is_none() && tagged.first_tag().is_none() {
        tagged.insert_tag(Tag::new(tagged.primary_tag_type()));
    }
    let tag = match tagged.primary_tag_mut() {
        Some(t) => t,
        None => tagged
            .first_tag_mut()
            .ok_or_else(|| AppError::Message("无法打开文件的标签区".into()))?,
    };

    set_or_remove(tag, Some(&patch.title), |t, s| t.set_title(s), |t| t.remove_title());
    set_or_remove(tag, patch.artist.as_deref(), |t, s| t.set_artist(s), |t| t.remove_artist());
    set_or_remove(tag, patch.album.as_deref(), |t, s| t.set_album(s), |t| t.remove_album());
    set_or_remove(tag, patch.genre.as_deref(), |t, s| t.set_genre(s), |t| t.remove_genre());
    match patch.year {
        Some(y) => tag.set_date(Timestamp { year: y.clamp(0, 9999) as u16, ..Default::default() }),
        None => tag.remove_date(),
    }
    match patch.track_no {
        Some(n) if n > 0 => tag.set_track(n as u32),
        _ => tag.remove_track(),
    }

    tagged.save_to_path(path, WriteOptions::default())?;
    Ok(())
}

/// 曲库同步：写回成功后更新 tracks（含新 mtime/size，增量扫描将按 unchanged 跳过），
/// 艺人/专辑按名 get-or-create，不再被引用的旧记录清理；FTS 触发器随 artist_id/title 更新刷新
pub(crate) fn apply_tag_to_db(
    conn: &mut rusqlite::Connection,
    track_id: i64,
    patch: &TagPatch,
) -> AppResult<()> {
    let (path, old_artist_id, old_album_id): (String, Option<i64>, Option<i64>) = conn.query_row(
        "SELECT path, artist_id, album_id FROM tracks WHERE id = ?1",
        params![track_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let artist_id = match patch.artist.as_deref().map(str::trim) {
        Some(a) if !a.is_empty() => Some(get_or_create_artist(conn, a)?),
        _ => None,
    };
    let album_id = match patch.album.as_deref().map(str::trim) {
        Some(al) if !al.is_empty() => Some(get_or_create_album(conn, al, artist_id)?),
        _ => None,
    };

    // 文件刚写回，读取新 mtime/size 让增量扫描按 unchanged 跳过
    let (mtime_ms, size) = match std::fs::metadata(&path) {
        Ok(md) => (
            md.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            md.len() as i64,
        ),
        Err(_) => (0, 0),
    };

    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE tracks SET title = ?1, artist_id = ?2, album_id = ?3, track_no = ?4, \
             year = ?5, genre = ?6, mtime_ms = ?7, size = ?8, updated_at = datetime('now') \
         WHERE id = ?9",
        params![
            patch.title.trim(),
            artist_id,
            album_id,
            patch.track_no,
            patch.year,
            patch.genre.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            mtime_ms,
            size,
            track_id,
        ],
    )?;
    // 旧记录若已无曲目引用则清理（专辑/艺人视图不留空壳）
    for (table, fk_col, old) in [("artists", "artist_id", old_artist_id), ("albums", "album_id", old_album_id)] {
        if let Some(old_id) = old {
            let referenced: i64 = tx.query_row(
                &format!("SELECT COUNT(*) FROM tracks WHERE {fk_col} = ?1"),
                params![old_id],
                |r| r.get(0),
            )?;
            if referenced == 0 {
                tx.execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![old_id])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

fn get_or_create_artist(conn: &rusqlite::Connection, name: &str) -> AppResult<i64> {
    conn.execute("INSERT OR IGNORE INTO artists(name) VALUES (?1)", params![name])?;
    Ok(conn.query_row("SELECT id FROM artists WHERE name = ?1", params![name], |r| r.get(0))?)
}

/// albums.artist 存艺人名文本（M0 结构），艺人已清除时留空
fn get_or_create_album(conn: &rusqlite::Connection, name: &str, artist_id: Option<i64>) -> AppResult<i64> {
    let artist_name = match artist_id {
        Some(id) => conn
            .query_row("SELECT name FROM artists WHERE id = ?1", params![id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap_or_default(),
        None => String::new(),
    };
    conn.execute(
        "INSERT OR IGNORE INTO albums(name, artist) VALUES (?1, ?2)",
        params![name, artist_name],
    )?;
    Ok(conn.query_row(
        "SELECT id FROM albums WHERE name = ?1 AND artist = ?2",
        params![name, artist_name],
        |r| r.get(0),
    )?)
}

#[tauri::command]
#[specta::specta]
pub async fn tag_update(
    state: State<'_, AppState>,
    track_id: i32,
    patch: TagPatch,
) -> Result<Track, AppErrorDto> {
    if patch.title.trim().is_empty() {
        return Err(AppError::Message("标题不能为空".into()).into());
    }
    let (path, patch) = {
        let conn = state.conn.lock();
        let path: String = conn
            .query_row(
                "SELECT path FROM tracks WHERE id = ?1",
                params![i64::from(track_id)],
                |r| r.get(0),
            )
            .map_err(AppError::from)?;
        (path, patch)
    };
    write_tags(Path::new(&path), &patch).map_err(AppErrorDto::from)?;
    {
        let mut conn = state.conn.lock();
        apply_tag_to_db(&mut conn, i64::from(track_id), &patch).map_err(AppErrorDto::from)?;
    }
    let conn = state.conn.lock();
    let sql = format!("{TRACK_SELECT} WHERE t.id = ?1");
    let mut stmt = conn.prepare(&sql).map_err(AppError::from)?;
    stmt.query_row(params![i64::from(track_id)], row_to_track)
        .map_err(AppError::from)
        .map_err(AppErrorDto::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn patch(title: &str, artist: Option<&str>, album: Option<&str>, genre: Option<&str>, year: Option<i32>, track_no: Option<i32>) -> TagPatch {
        TagPatch {
            title: title.into(),
            artist: artist.map(str::to_string),
            album: album.map(str::to_string),
            genre: genre.map(str::to_string),
            year,
            track_no,
        }
    }

    #[test]
    fn 标签编辑同步曲库并清理孤儿() {
        let mut conn = db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO artists(id, name) VALUES (1, '旧艺人');
             INSERT INTO albums(id, name, artist) VALUES (1, '旧专辑', '旧艺人');
             INSERT INTO tracks(id, path, folder_id, title, artist_id, album_id) VALUES
               (1, 'a.mp3', 1, 'A', 1, 1), (2, 'b.mp3', 1, 'B', 1, 1);",
        )
        .unwrap();

        apply_tag_to_db(&mut conn, 1, &patch("新标题", Some("新艺人"), Some("新专辑"), Some("J-Pop"), Some(2024), Some(3)))
            .unwrap();

        let row: (String, String, String, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT t.title, ar.name, al.name, t.year, t.track_no FROM tracks t \
                 LEFT JOIN artists ar ON ar.id = t.artist_id \
                 LEFT JOIN albums al ON al.id = t.album_id WHERE t.id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(row, ("新标题".into(), "新艺人".into(), "新专辑".into(), Some(2024), Some(3)));

        // 曲目 2 仍引用旧艺人/专辑，不清理
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 2);
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 2);

        // 曲目 2 也改走后，旧记录成孤儿被清理
        apply_tag_to_db(&mut conn, 2, &patch("B2", Some("新艺人"), Some("新专辑"), None, None, None)).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);

        // FTS 触发器随 artist_id/title 更新刷新：可按新艺人搜到
        let hit: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks_fts WHERE tracks_fts MATCH '新艺人'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(hit, 2);
    }

    #[test]
    fn 清除字段置空并拒绝空标题() {
        let mut conn = db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO artists(id, name) VALUES (1, 'A1');
             INSERT INTO albums(id, name, artist) VALUES (1, 'AL', 'A1');
             INSERT INTO tracks(id, path, folder_id, title, artist_id, album_id, genre, year) VALUES
               (1, 'a.mp3', 1, 'T', 1, 1, 'Rock', 2000);",
        )
        .unwrap();
        apply_tag_to_db(&mut conn, 1, &patch("T2", Some("  "), Some(""), None, None, None)).unwrap();
        let row: (Option<i64>, Option<i64>, Option<String>, Option<i64>) = conn
            .query_row("SELECT artist_id, album_id, genre, year FROM tracks WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .unwrap();
        assert_eq!(row, (None, None, None, None));
        // 空壳艺人/专辑被清理
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
        assert!(conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get::<_, i64>(0)).unwrap() == 0);
    }
}
