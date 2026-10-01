//! 重复曲目检测与清理（M6）：按 归一化标题+艺人 分组、组内时长 ±2s 聚类；
//! 清理 = 源文件移入系统回收站（trash crate，可恢复）+ 曲库记录删除（评审决议）。
//! 仅删曲库记录会被增量扫描重新扫回，故必须删除源文件。

use rusqlite::params;
use serde::Deserialize;
use tauri::State;

use crate::commands::tracks::{row_to_track, TRACK_SELECT};
use crate::error::{AppErrorDto, AppResult};
use crate::models::{DuplicateGroup, DuplicateItem, DuplicateResolve, Track};
use crate::AppState;

/// 分组输入行（与 DTO 解耦，便于单测）
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DupRow {
    pub title: String,
    pub artist: String,
    pub duration_sec: f64,
}

/// 归一化分组键：去首尾空白 + 小写
fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// 按键分组 + 组内按时长排序贪心聚类（相邻差 ≤ 2s 归为同簇）。
/// 返回每簇的行下标列表（仅 ≥ 2 首的簇），簇按 艺人/标题 排序保证输出稳定。
pub(crate) fn group_duplicates(rows: &[DupRow]) -> Vec<Vec<usize>> {
    const DURATION_TOLERANCE: f64 = 2.0;

    let mut by_key: std::collections::HashMap<(String, String), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        let key = (norm(&row.title), norm(&row.artist));
        if key.0.is_empty() || key.1.is_empty() {
            continue;
        }
        by_key.entry(key).or_default().push(i);
    }

    let mut keys: Vec<_> = by_key.into_iter().collect();
    keys.sort_by(|a, b| (&a.0.1, &a.0.0).cmp(&(&b.0.1, &b.0.0)));

    let mut out = Vec::new();
    for (_, mut indices) in keys {
        if indices.len() < 2 {
            continue;
        }
        indices.sort_by(|&a, &b| {
            rows[a]
                .duration_sec
                .partial_cmp(&rows[b].duration_sec)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut cluster: Vec<usize> = vec![indices[0]];
        for &idx in &indices[1..] {
            let base = rows[cluster[0]].duration_sec;
            if (rows[idx].duration_sec - base).abs() <= DURATION_TOLERANCE {
                cluster.push(idx);
            } else {
                if cluster.len() >= 2 {
                    out.push(std::mem::take(&mut cluster));
                } else {
                    cluster.clear();
                }
                cluster = vec![idx];
            }
        }
        if cluster.len() >= 2 {
            out.push(cluster);
        }
    }
    out
}

#[tauri::command]
#[specta::specta]
pub async fn duplicates_scan(state: State<'_, AppState>) -> Result<Vec<DuplicateGroup>, AppErrorDto> {
    scan_groups(&state.conn.lock()).map_err(AppErrorDto::from)
}

/// 重复分组（同步纯逻辑，单测直连内存库调用）
pub(crate) fn scan_groups(conn: &rusqlite::Connection) -> AppResult<Vec<DuplicateGroup>> {
    // 附加列必须进 SELECT 列表（FROM 之前）：TRACK_SELECT 以 FROM/JOIN 结尾，
    // 直接尾拼会把 , t.size 解析为交叉连接的"表名"（no such table: t.size）
    let base = TRACK_SELECT
        .replacen(" FROM tracks t", ", t.size, t.created_at FROM tracks t", 1)
        ;
    debug_assert!(base.contains(", t.size, t.created_at FROM"));
    let sql = format!("{base} ORDER BY t.created_at, t.id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        let track = row_to_track(r)?;
        Ok((track, r.get::<_, f64>(12)?, r.get::<_, String>(13)?))
    })?;
    let all: Vec<(Track, f64, String)> = rows.collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let dup_rows: Vec<DupRow> = all
        .iter()
        .map(|(t, _, _)| DupRow {
            title: t.title.clone(),
            artist: t.artist.clone().unwrap_or_default(),
            duration_sec: t.duration_sec.unwrap_or(0.0),
        })
        .collect();

    let mut groups = Vec::new();
    for cluster in group_duplicates(&dup_rows) {
        // 簇内按 created_at 升序（扫描查询已排序，这里显式再排一次保险）
        let mut items: Vec<DuplicateItem> = cluster
            .into_iter()
            .map(|idx| {
                let (track, size, created_at) = &all[idx];
                DuplicateItem {
                    track: track.clone(),
                    size: *size,
                    created_at: created_at.clone(),
                }
            })
            .collect();
        items.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let artist = items[0].track.artist.clone().unwrap_or_default();
        let title = items[0].track.title.clone();
        groups.push(DuplicateGroup { title, artist, items });
    }
    Ok(groups)
}

/// 清理重复项：源文件移入回收站成功后删除曲库记录（级联清 歌单曲目/收藏/播放历史）
#[tauri::command]
#[specta::specta]
pub async fn duplicate_resolve(
    state: State<'_, AppState>,
    keep_id: i32,
    remove_ids: Vec<i32>,
) -> Result<DuplicateResolve, AppErrorDto> {
    let targets: Vec<i64> = remove_ids
        .into_iter()
        .filter(|id| *id != keep_id)
        .map(i64::from)
        .collect();

    let mut removed = 0u32;
    let mut failed = 0u32;
    let mut first_error: Option<String> = None;

    for id in targets {
        let result = resolve_one(&state, id);
        match result {
            Ok(()) => removed += 1,
            Err(e) => {
                failed += 1;
                first_error.get_or_insert_with(|| e.to_string());
            }
        }
    }
    Ok(DuplicateResolve { removed, failed, first_error })
}

fn resolve_one(state: &AppState, id: i64) -> AppResult<()> {
    let path: Option<String> = {
        let conn = state.conn.lock();
        conn.query_row("SELECT path FROM tracks WHERE id = ?1", params![id], |r| r.get(0))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?
    };
    let Some(path) = path else {
        return Ok(()); // 已不存在视为成功
    };
    // 先移文件（失败则保留记录，避免扫描把记录重建后出现孤儿状态不一致）
    trash::delete(&path)?;
    let conn = state.conn.lock();
    conn.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str, artist: &str, duration: f64) -> DupRow {
        DupRow { title: title.into(), artist: artist.into(), duration_sec: duration }
    }

    #[test]
    fn 同曲不同目录时长相近归为一组() {
        let rows = vec![
            row("First Love", "宇多田ヒカル", 254.0),
            row("first love ", "宇多田ヒカル", 254.8), // 大小写/空格归一化
            row("FIRST LOVE", "宇多田ヒカル", 253.2),
            row("Other Song", "宇多田ヒカル", 200.0),
        ];
        let groups = group_duplicates(&rows);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![2, 0, 1]); // 组内按时长升序
    }

    #[test]
    fn 时长差超过容差拆分为不同组() {
        let rows = vec![
            row("Song", "A", 200.0),
            row("Song", "A", 205.0), // 差 5s > 2s
            row("Song", "A", 205.5), // 与 205 差 0.5s
        ];
        let groups = group_duplicates(&rows);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![1, 2]); // 200 单独不成组，205/205.5 成组
    }

    #[test]
    fn 不同艺人或单曲不成组() {
        let rows = vec![
            row("Same", "A", 100.0),
            row("Same", "B", 100.0),
            row("Unique", "A", 100.0),
        ];
        assert!(group_duplicates(&rows).is_empty());
    }

    #[test]
    fn duplicates_scan在真实迁移库上分组() {
        use crate::db;
        let conn = db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO artists(id, name) VALUES (1, '宇多田ヒカル');
             INSERT INTO tracks(path, folder_id, title, artist_id, duration_sec, size, created_at) VALUES
               ('a.mp3', 1, 'First Love', 1, 254.0, 100, '2026-09-01 00:00:00'),
               ('b.mp3', 1, 'first love', 1, 254.5, 120, '2026-09-02 00:00:00'),
               ('c.mp3', 1, 'Other', 1, 100.0, 80, '2026-09-03 00:00:00');",
        )
        .unwrap();
        let groups = scan_groups(&conn).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].items.len(), 2);
        // 保留候选 = 添加时间最早
        assert_eq!(groups[0].items[0].track.path, "a.mp3");
        assert_eq!(groups[0].items[0].size, 100.0);
    }

    #[test]
    fn 多组输出按键排序稳定() {
        let rows = vec![
            row("B曲", "乙", 50.0),
            row("B曲", "乙", 50.5),
            row("A曲", "甲", 60.0),
            row("A曲", "甲", 60.5),
        ];
        let groups = group_duplicates(&rows);
        assert_eq!(groups.len(), 2);
        // 按键（艺人,标题）Unicode 序：乙(U+4E59) < 甲(U+7532)，乙组在前
        assert_eq!(groups[0], vec![0, 1]);
        assert_eq!(groups[1], vec![2, 3]);
    }
}
