use rusqlite::params;
use tauri::State;

use crate::error::{AppErrorDto, AppResult};
use crate::models::TrackPage;
use crate::AppState;

use super::tracks::{row_to_track, TRACK_SELECT};

/// 转义 LIKE 通配符，用户输入按字面量匹配
fn like_pattern(q: &str) -> String {
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

/// 即时搜索：标题/艺人/专辑子串匹配。
/// 走 tracks_fts（trigram 虚表）：≥3 字符的模式由 SQLite 自动用 trigram 索引加速，
/// 更短的查询回退虚表全扫描；按默认 歌手/专辑/曲号 排序，分页返回。
fn query_search(
    state: &AppState,
    q: &str,
    page: Option<u32>,
    page_size: Option<u32>,
) -> AppResult<TrackPage> {
    let page = page.unwrap_or(1).max(1) as i64;
    let size = page_size.unwrap_or(200).clamp(1, 1000) as i64;
    let offset = (page - 1) * size;
    let pattern = like_pattern(q);

    let conn = state.conn.lock();
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tracks t WHERE t.id IN (\
            SELECT rowid FROM tracks_fts \
            WHERE title LIKE ?1 ESCAPE '\\' OR artist LIKE ?1 ESCAPE '\\' OR album LIKE ?1 ESCAPE '\\')",
        params![pattern],
        |r| r.get(0),
    )?;
    let sql = format!(
        "{TRACK_SELECT} WHERE t.id IN (\
            SELECT rowid FROM tracks_fts \
            WHERE title LIKE ?1 ESCAPE '\\' OR artist LIKE ?1 ESCAPE '\\' OR album LIKE ?1 ESCAPE '\\') \
         ORDER BY ar.name COLLATE NOCASE, al.name COLLATE NOCASE, t.disc, t.track_no, t.title COLLATE NOCASE \
         LIMIT ?2 OFFSET ?3"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params![pattern, size, offset], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TrackPage { items, total: total as i32 })
}

#[tauri::command]
#[specta::specta]
pub async fn search_tracks(
    state: State<'_, AppState>,
    q: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<TrackPage, AppErrorDto> {
    query_search(&state, q.trim(), page, page_size).map_err(AppErrorDto::from)
}

#[cfg(test)]
mod tests {
    use crate::db;

    fn seed(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO artists(id, name) VALUES (1, '周杰伦');
             INSERT INTO albums(id, name, artist) VALUES (1, '叶惠美', '周杰伦');
             INSERT INTO tracks(path, title, artist_id, album_id, folder_id) VALUES
               ('a.mp3', '晴天', 1, 1, 1),
               ('b.mp3', '以父之名', 1, 1, 1),
               ('c.mp3', 'Night Rain', NULL, NULL, 1);",
        )
        .unwrap();
    }

    #[test]
    fn 中文子串与英文大小写不敏感搜索() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        for (q, expect) in [("晴", 1usize), ("晴天", 1), ("周杰", 2), ("rain", 1), ("RAIN", 1), ("不存在", 0)] {
            let pattern = super::like_pattern(q);
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM tracks t WHERE t.id IN (\
                        SELECT rowid FROM tracks_fts \
                        WHERE title LIKE ?1 ESCAPE '\\' OR artist LIKE ?1 ESCAPE '\\' OR album LIKE ?1 ESCAPE '\\')",
                    [&pattern],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n as usize, expect, "查询 {q} 期望 {expect} 条");
        }
    }

    #[test]
    fn like通配符按字面量匹配() {
        assert_eq!(super::like_pattern("50%"), "%50\\%%");
        assert_eq!(super::like_pattern("a_b"), "%a\\_b%");
        assert_eq!(super::like_pattern("a\\b"), "%a\\\\b%");
    }

    /// 旧库（0001）已有曲目 → 升级迁移 → 回填后搜索必须命中既有曲目
    #[test]
    fn 升级迁移回填搜索索引() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(include_str!("../../migrations/0001_init.sql")).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO tracks(path, title, folder_id) VALUES ('old.mp3', '旧歌', 1);",
        )
        .unwrap();
        crate::db::migrate::run(&conn).unwrap();
        let pattern = super::like_pattern("旧歌");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tracks_fts WHERE title LIKE ?1 ESCAPE '\\'",
                [&pattern],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "迁移回填后既有曲目必须可搜到");
    }
}
