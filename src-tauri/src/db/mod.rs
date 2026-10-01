pub(crate) mod migrate;

use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

/// 打开连接：WAL（多连接并发）+ 外键（级联删除）+ 顺序迁移
pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate::run(&conn)?;
    Ok(conn)
}

/// 内存库（单测用）：与磁盘库同一套迁移
pub fn open_in_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate::run(&conn)?;
    Ok(conn)
}

/// 曲库来源目录：(folder_id, path)
pub fn folder_rows(conn: &Connection) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare("SELECT id, path FROM folders ORDER BY id")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}
