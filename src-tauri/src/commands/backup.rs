//! 全量备份导出/恢复（M7，技术设计文档 §5 backup_*）：
//! 歌单 + 智能歌单 + 收藏 + 播放历史 + 设置 → 单 JSON 文件。
//! 曲库本身不进备份——音频文件在本地磁盘，恢复时按路径归一化匹配
//! （复用 m3u8::normalize_path，大小写/斜杠差异不影响命中），未命中跳过并计数报告。
//! 恢复为合并语义：同名歌单追加曲目、已有收藏/历史去重、设置按键覆盖。

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::error::{AppError, AppErrorDto, AppResult};
use crate::library::m3u8;
use crate::models::SmartRule;
use crate::AppState;

/// 备份文件格式版本（结构不兼容时递增）
const FORMAT_VERSION: i64 = 1;
/// 不进备份的设置键：主窗口位置尺寸与显示器绑定，属于机器状态而非用户偏好
const EXCLUDED_SETTING_KEYS: &[&str] = &["window"];
/// 播放历史与 history_add 同上限，避免备份无限膨胀
const HISTORY_LIMIT: i64 = 5000;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupPlaylist {
    pub name: String,
    /// 曲目绝对路径，按歌单内顺序
    pub tracks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupSmartPlaylist {
    pub name: String,
    pub rules: Vec<SmartRule>,
    pub track_limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupHistoryEntry {
    pub path: String,
    /// SQLite datetime 文本（"YYYY-MM-DD HH:MM:SS"），原样往返
    pub played_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupSetting {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupData {
    pub version: i64,
    pub exported_at: String,
    pub playlists: Vec<BackupPlaylist>,
    pub smart_playlists: Vec<BackupSmartPlaylist>,
    /// 收藏曲目路径（按收藏时间先后）
    pub favorites: Vec<String>,
    pub history: Vec<BackupHistoryEntry>,
    pub settings: Vec<BackupSetting>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupExportResult {
    pub playlists: u32,
    pub tracks: u32,
    pub smart_playlists: u32,
    pub favorites: u32,
    pub history: u32,
}

/// 恢复报告：skipped 汇总所有按路径未命中曲库的引用（歌单曲目/收藏/历史）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreResult {
    pub playlists_created: u32,
    /// 同名歌单合并追加曲目的个数
    pub playlists_merged: u32,
    pub playlist_tracks_added: u32,
    pub smart_playlists_created: u32,
    /// 同名已存在或规则非法而跳过的智能歌单个数
    pub smart_playlists_skipped: u32,
    pub favorites_added: u32,
    pub history_added: u32,
    pub tracks_skipped: u32,
    pub settings_restored: u32,
}

fn export_data(conn: &Connection) -> AppResult<BackupData> {
    // 复用 SQLite 的 datetime('now')（UTC "YYYY-MM-DD HH:MM:SS"），与库内时间格式一致
    let exported_at: String = conn
        .query_row("SELECT datetime('now')", [], |r| r.get(0))
        .unwrap_or_default();
    let mut playlists = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, name FROM playlists ORDER BY id")?;
        let rows =
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, name) = row?;
            let mut stmt = conn.prepare(
                "SELECT t.path FROM playlist_tracks pt \
                 JOIN tracks t ON t.id = pt.track_id \
                 WHERE pt.playlist_id = ?1 ORDER BY pt.position",
            )?;
            let tracks = stmt
                .query_map(params![id], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            playlists.push(BackupPlaylist { name, tracks });
        }
    }

    let mut smart_playlists = Vec::new();
    {
        let mut stmt =
            conn.prepare("SELECT name, rules, track_limit FROM smart_playlists ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?;
        for row in rows {
            let (name, rules_raw, limit) = row?;
            // 库内 rules 损坏（非法 JSON）时跳过该智能歌单，不阻断导出
            let rules = match serde_json::from_str::<Vec<SmartRule>>(&rules_raw) {
                Ok(rules) => rules,
                Err(_) => continue,
            };
            smart_playlists.push(BackupSmartPlaylist {
                name,
                rules,
                track_limit: limit.map(|v| v as i32),
            });
        }
    }

    let favorites = {
        let mut stmt = conn.prepare(
            "SELECT t.path FROM favorites f JOIN tracks t ON t.id = f.track_id \
             ORDER BY f.created_at, f.rowid",
        )?;
        let items = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        items
    };

    let history = {
        let mut stmt = conn.prepare(
            "SELECT t.path, h.played_at FROM play_history h \
             JOIN tracks t ON t.id = h.track_id \
             ORDER BY h.id DESC LIMIT ?1",
        )?;
        let mut items = stmt
            .query_map(params![HISTORY_LIMIT], |r| {
                Ok(BackupHistoryEntry { path: r.get(0)?, played_at: r.get(1)? })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // id DESC 取的是最近记录，导出按时间正序便于阅读
        items.reverse();
        items
    };

    let mut settings: Vec<BackupSetting> = {
        let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |r| {
            Ok(BackupSetting { key: r.get(0)?, value: r.get(1)? })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    settings.retain(|s| !EXCLUDED_SETTING_KEYS.contains(&s.key.as_str()));

    Ok(BackupData {
        version: FORMAT_VERSION,
        exported_at,
        playlists,
        smart_playlists,
        favorites,
        history,
        settings,
    })
}

fn restore_data(conn: &mut Connection, data: &BackupData) -> AppResult<BackupRestoreResult> {
    let path_to_id: HashMap<String, i64> = {
        let mut stmt = conn.prepare("SELECT id, path FROM tracks")?;
        let rows =
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(id, path)| (m3u8::normalize_path(&path), id))
            .collect()
    };

    let mut result = BackupRestoreResult {
        playlists_created: 0,
        playlists_merged: 0,
        playlist_tracks_added: 0,
        smart_playlists_created: 0,
        smart_playlists_skipped: 0,
        favorites_added: 0,
        history_added: 0,
        tracks_skipped: 0,
        settings_restored: 0,
    };

    let tx = conn.transaction()?;

    // 设置：按备份中存在的键覆盖（merge），不删除备份外的键
    for setting in &data.settings {
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![setting.key, setting.value],
        )?;
        result.settings_restored += 1;
    }

    // 歌单：同名合并（追加未命中的曲目到末尾），否则新建
    for playlist in &data.playlists {
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM playlists WHERE name = ?1",
                params![playlist.name],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let playlist_id = match existing {
            Some(id) => {
                result.playlists_merged += 1;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO playlists(name, created_at) VALUES (?1, ?2)",
                    params![playlist.name, data.exported_at],
                )?;
                result.playlists_created += 1;
                tx.last_insert_rowid()
            }
        };
        let mut pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |r| r.get::<_, i64>(0),
        )? + 1;
        for path in &playlist.tracks {
            let Some(track_id) = path_to_id.get(&m3u8::normalize_path(path)) else {
                result.tracks_skipped += 1;
                continue;
            };
            let n = tx.execute(
                "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position) \
                 VALUES (?1, ?2, ?3)",
                params![playlist_id, track_id, pos],
            )?;
            // OR IGNORE 命中已有曲目不计入新增，但 position 仍前移保持顺序语义
            pos += 1;
            result.playlist_tracks_added += n as u32;
        }
    }

    // 智能歌单：同名跳过；规则非法（备份被篡改/跨版本）跳过，不阻断其余恢复
    for smart in &data.smart_playlists {
        let exists: bool = tx
            .query_row(
                "SELECT COUNT(*) FROM smart_playlists WHERE name = ?1",
                params![smart.name],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)?;
        if exists {
            result.smart_playlists_skipped += 1;
            continue;
        }
        if let Err(e) = crate::commands::smart_playlists::where_clause(&smart.rules) {
            eprintln!("[backup] 智能歌单「{}」规则非法，已跳过: {e}", smart.name);
            result.smart_playlists_skipped += 1;
            continue;
        }
        tx.execute(
            "INSERT INTO smart_playlists(name, rules, track_limit) VALUES (?1, ?2, ?3)",
            params![
                smart.name,
                serde_json::to_string(&smart.rules)?,
                smart.track_limit.map(i64::from)
            ],
        )?;
        result.smart_playlists_created += 1;
    }

    // 收藏：去重插入
    for path in &data.favorites {
        let Some(track_id) = path_to_id.get(&m3u8::normalize_path(path)) else {
            result.tracks_skipped += 1;
            continue;
        };
        let n = tx.execute(
            "INSERT OR IGNORE INTO favorites(track_id) VALUES (?1)",
            params![track_id],
        )?;
        result.favorites_added += n as u32;
    }

    // 历史：同曲同刻去重（重复恢复同一备份不产生重复历史）
    for entry in &data.history {
        let Some(track_id) = path_to_id.get(&m3u8::normalize_path(&entry.path)) else {
            result.tracks_skipped += 1;
            continue;
        };
        let n = tx.execute(
            "INSERT INTO play_history(track_id, played_at) \
             SELECT ?1, ?2 WHERE NOT EXISTS (\
                 SELECT 1 FROM play_history WHERE track_id = ?1 AND played_at = ?2)",
            params![track_id, entry.played_at],
        )?;
        result.history_added += n as u32;
    }

    tx.commit()?;
    Ok(result)
}

/// 导出全量备份到 JSON 文件，返回各部分计数（设置页展示用）
#[tauri::command]
#[specta::specta]
pub async fn backup_export(
    state: State<'_, AppState>,
    path: String,
) -> Result<BackupExportResult, AppErrorDto> {
    let data = {
        let conn = state.conn.lock();
        export_data(&conn).map_err(AppErrorDto::from)?
    };
    let text = serde_json::to_string_pretty(&data).map_err(AppError::from)?;
    std::fs::write(&path, text).map_err(AppError::from)?;
    Ok(BackupExportResult {
        playlists: data.playlists.len() as u32,
        tracks: data.playlists.iter().map(|p| p.tracks.len() as u32).sum(),
        smart_playlists: data.smart_playlists.len() as u32,
        favorites: data.favorites.len() as u32,
        history: data.history.len() as u32,
    })
}

/// 从 JSON 文件恢复备份（合并语义），返回恢复报告；
/// 设置恢复后即时重注册各全局快捷键（覆盖"新环境恢复"场景，无需重启）
#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<BackupRestoreResult, AppErrorDto> {
    let bytes = std::fs::read(&path).map_err(AppError::from)?;
    let data: BackupData = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Message(format!("备份文件格式无效: {e}")))?;
    if data.version > FORMAT_VERSION {
        return Err(AppError::Message(format!(
            "备份文件版本过新（v{}），请升级应用后再恢复",
            data.version
        ))
        .into());
    }
    let result = {
        let mut conn = state.conn.lock();
        restore_data(&mut conn, &data).map_err(AppErrorDto::from)?
    };
    // 恢复的设置可能改动了快捷键配置：解绑旧键并按新配置重注册（失败仅告警）
    crate::overlay::reapply_shortcut(&app);
    crate::shortcuts::apply_all(&app);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn insert_track(conn: &Connection, path: &str, title: &str) -> i64 {
        let folder: i64 = conn
            .query_row("SELECT id FROM folders WHERE path = 'C:\\music'", [], |r| r.get(0))
            .unwrap_or_else(|_| {
                conn.execute("INSERT INTO folders(path) VALUES ('C:\\music')", []).unwrap();
                conn.last_insert_rowid()
            });
        conn.execute(
            "INSERT INTO tracks(path, folder_id, title) VALUES (?1, ?2, ?3)",
            params![path, folder, title],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn 导出后恢复完整还原歌单收藏历史与设置() {
        let conn = db::open_in_memory().unwrap();
        let id1 = insert_track(&conn, r"C:\music\a.mp3", "A");
        let id2 = insert_track(&conn, r"C:\music\b.flac", "B");
        conn.execute("INSERT INTO playlists(name) VALUES ('我的歌单')", []).unwrap();
        conn.execute(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position) VALUES (1, ?1, 0), (1, ?2, 1)",
            params![id1, id2],
        )
        .unwrap();
        conn.execute("INSERT INTO favorites(track_id) VALUES (?1)", params![id2]).unwrap();
        conn.execute("INSERT INTO play_history(track_id, played_at) VALUES (?1, '2026-10-01 12:00:00')", params![id1])
            .unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES ('appearance', '{\"theme\":\"dark\"}'), ('window', '{\"x\":1}')",
            [],
        )
        .unwrap();

        let data = export_data(&conn).unwrap();
        assert_eq!(data.playlists.len(), 1);
        assert_eq!(data.playlists[0].tracks, vec![r"C:\music\a.mp3", r"C:\music\b.flac"]);
        // window 键不进备份
        assert!(data.settings.iter().all(|s| s.key != "window"));

        // 模拟新环境：清空用户数据后按路径恢复
        let mut fresh = db::open_in_memory().unwrap();
        insert_track(&fresh, r"C:\music\a.mp3", "A");
        insert_track(&fresh, r"C:\MUSIC\B.FLAC", "B"); // 大小写不同也应命中
        let report = restore_data(&mut fresh, &data).unwrap();
        assert_eq!(report.playlists_created, 1);
        assert_eq!(report.playlist_tracks_added, 2);
        assert_eq!(report.favorites_added, 1);
        assert_eq!(report.history_added, 1);
        assert_eq!(report.tracks_skipped, 0);
        assert_eq!(report.settings_restored, 1);

        let restored: String = fresh
            .query_row("SELECT value FROM settings WHERE key = 'appearance'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(restored, "{\"theme\":\"dark\"}");
        let count: i64 = fresh
            .query_row("SELECT COUNT(*) FROM playlist_tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn 恢复未命中路径计数跳过且不阻断() {
        let conn = db::open_in_memory().unwrap();
        let id = insert_track(&conn, r"C:\music\a.mp3", "A");
        conn.execute("INSERT INTO favorites(track_id) VALUES (?1)", params![id]).unwrap();
        conn.execute("INSERT INTO playlists(name) VALUES ('P')", []).unwrap();
        conn.execute(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position) VALUES (1, ?1, 0)",
            params![id],
        )
        .unwrap();

        let mut data = export_data(&conn).unwrap();
        data.playlists[0].tracks.push(r"D:\gone.mp3".into());
        data.favorites.push(r"D:\gone2.mp3".into());
        data.history.push(BackupHistoryEntry {
            path: r"D:\gone3.mp3".into(),
            played_at: "2026-10-01 00:00:00".into(),
        });

        let mut fresh = db::open_in_memory().unwrap();
        insert_track(&fresh, r"C:\music\a.mp3", "A");
        let report = restore_data(&mut fresh, &data).unwrap();
        assert_eq!(report.playlist_tracks_added, 1);
        assert_eq!(report.favorites_added, 1);
        assert_eq!(report.tracks_skipped, 3);
    }

    #[test]
    fn 重复恢复同一备份不产生重复数据() {
        let conn = db::open_in_memory().unwrap();
        let id = insert_track(&conn, r"C:\music\a.mp3", "A");
        conn.execute("INSERT INTO playlists(name) VALUES ('P')", []).unwrap();
        conn.execute(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position) VALUES (1, ?1, 0)",
            params![id],
        )
        .unwrap();
        conn.execute("INSERT INTO favorites(track_id) VALUES (?1)", params![id]).unwrap();
        conn.execute(
            "INSERT INTO play_history(track_id, played_at) VALUES (?1, '2026-10-01 12:00:00')",
            params![id],
        )
        .unwrap();
        let data = export_data(&conn).unwrap();

        let mut fresh = db::open_in_memory().unwrap();
        let id2 = insert_track(&fresh, r"C:\music\a.mp3", "A");
        let first = restore_data(&mut fresh, &data).unwrap();
        let second = restore_data(&mut fresh, &data).unwrap();

        assert_eq!(first.playlists_created, 1);
        assert_eq!(second.playlists_created, 0);
        assert_eq!(second.playlists_merged, 1);
        assert_eq!(second.playlist_tracks_added, 0, "同名曲目在歌单中已存在");
        assert_eq!(second.favorites_added, 0);
        assert_eq!(second.history_added, 0);
        let count: i64 = fresh
            .query_row("SELECT COUNT(*) FROM play_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let _ = id2;
    }

    #[test]
    fn 智能歌单同名跳过且非法规则不阻断() {
        let conn = db::open_in_memory().unwrap();
        insert_track(&conn, r"C:\music\a.mp3", "A");
        let rules = r#"[{"field":"playCount","op":"gte","num":1,"text":null}]"#;
        conn.execute(
            "INSERT INTO smart_playlists(name, rules) VALUES ('常听', ?1)",
            params![rules],
        )
        .unwrap();
        let mut data = export_data(&conn).unwrap();
        assert_eq!(data.smart_playlists.len(), 1);
        // 追加一个"可反序列化但条件组合非法"的智能歌单（genre 不支持 gte）
        data.smart_playlists.push(BackupSmartPlaylist {
            name: "坏的".into(),
            rules: serde_json::from_str(r#"[{"field":"genre","op":"gte","num":1,"text":null}]"#)
                .unwrap(),
            track_limit: None,
        });

        let mut fresh = db::open_in_memory().unwrap();
        insert_track(&fresh, r"C:\music\a.mp3", "A");
        fresh
            .execute(
                "INSERT INTO smart_playlists(name, rules) VALUES ('常听', ?1)",
                params![rules],
            )
            .unwrap();
        let report = restore_data(&mut fresh, &data).unwrap();
        assert_eq!(report.smart_playlists_created, 0);
        assert_eq!(report.smart_playlists_skipped, 2);
    }
}
