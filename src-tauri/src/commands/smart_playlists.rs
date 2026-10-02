//! 智能歌单（M4）：规则式、实时生成不物化（技术设计文档 §4）。
//! 规则条件 = 播放次数 / 最近播放 / 流派 / 年份 / 添加时间 + 数量上限。

use rusqlite::types::Value as SqlValue;
use rusqlite::{params, Connection};
use tauri::State;

use crate::error::{AppError, AppErrorDto, AppResult};
use crate::models::{SmartField, SmartOp, SmartPlaylist, SmartRule, Track};
use crate::AppState;

use super::tracks::{row_to_track, TRACK_SELECT};

const MAX_RULES: usize = 10;
const MAX_LIMIT: i64 = 1000;

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Message("歌单名不能为空".into()));
    }
    Ok(name.chars().take(100).collect())
}

/// LIKE 通配符转义（用户输入的 % _ \ 按字面匹配）
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// 数值条件取值：有限、非负、上限防滥用
fn rule_num(rule: &SmartRule, max: f64) -> AppResult<i64> {
    let n = rule
        .num
        .ok_or_else(|| AppError::Message("规则缺少数值".into()))?;
    if !n.is_finite() || n < 0.0 || n > max {
        return Err(AppError::Message("规则数值超出范围".into()));
    }
    Ok(n as i64)
}

/// 单条规则 → (SQL 条件片段, 绑定参数)。非法 field/op 组合与取值直接报错。
pub(crate) fn rule_condition(rule: &SmartRule) -> AppResult<(String, Vec<SqlValue>)> {
    use SmartField as F;
    use SmartOp as O;
    let invalid = || AppError::Message("规则条件不合法".into());
    match (rule.field, rule.op) {
        (F::PlayCount, O::Gte | O::Lte) => {
            let n = rule_num(rule, 1_000_000_000.0)?;
            let op = if rule.op == O::Gte { ">=" } else { "<=" };
            Ok((
                format!("COALESCE(h.play_count, 0) {op} ?"),
                vec![SqlValue::Integer(n)],
            ))
        }
        (F::LastPlayed, O::WithinDays) => {
            let n = rule_num(rule, 36_500.0)?;
            Ok((
                "h.last_played >= datetime('now', ?)".into(),
                vec![SqlValue::Text(format!("-{n} days"))],
            ))
        }
        (F::Genre, O::Eq) => {
            let text = rule.text.as_deref().ok_or_else(invalid)?.trim();
            if text.is_empty() {
                return Err(invalid());
            }
            Ok((
                "t.genre = ? COLLATE NOCASE".into(),
                vec![SqlValue::Text(text.into())],
            ))
        }
        (F::Genre, O::Contains) => {
            let text = rule.text.as_deref().ok_or_else(invalid)?.trim();
            if text.is_empty() {
                return Err(invalid());
            }
            Ok((
                r"t.genre LIKE ? ESCAPE '\'".into(),
                vec![SqlValue::Text(format!("%{}%", escape_like(text)))],
            ))
        }
        (F::Year, O::Gte | O::Lte) => {
            let n = rule_num(rule, 3000.0)?;
            let op = if rule.op == O::Gte { ">=" } else { "<=" };
            Ok((format!("t.year {op} ?"), vec![SqlValue::Integer(n)]))
        }
        (F::AddedAt, O::WithinDays) => {
            let n = rule_num(rule, 36_500.0)?;
            Ok((
                "t.created_at >= datetime('now', ?)".into(),
                vec![SqlValue::Text(format!("-{n} days"))],
            ))
        }
        _ => Err(invalid()),
    }
}

/// 规则集 → WHERE 子句（无规则 = 全部曲目）
pub(crate) fn where_clause(rules: &[SmartRule]) -> AppResult<(String, Vec<SqlValue>)> {
    if rules.len() > MAX_RULES {
        return Err(AppError::Message(format!("规则最多 {MAX_RULES} 条")));
    }
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    for rule in rules {
        let (cond, mut vals) = rule_condition(rule)?;
        clauses.push(cond);
        values.append(&mut vals);
    }
    Ok((if clauses.is_empty() { "1=1".into() } else { clauses.join(" AND ") }, values))
}

/// 校验数量上限
fn clean_limit(track_limit: Option<i32>) -> AppResult<Option<i64>> {
    match track_limit {
        None => Ok(None),
        Some(n) if n >= 1 && i64::from(n) <= MAX_LIMIT => Ok(Some(i64::from(n))),
        Some(_) => Err(AppError::Message(format!("数量上限须在 1–{MAX_LIMIT} 之间"))),
    }
}

/// 智能歌单实时列表查询（播放次数/最近播放经 play_history 聚合子查询）
fn query_tracks(conn: &Connection, rules: &[SmartRule], track_limit: Option<i32>) -> AppResult<Vec<Track>> {
    let (where_sql, values) = where_clause(rules)?;
    let mut sql = format!(
        "{TRACK_SELECT} \
         LEFT JOIN (SELECT track_id, COUNT(*) AS play_count, MAX(played_at) AS last_played \
                    FROM play_history GROUP BY track_id) h ON h.track_id = t.id \
         WHERE {where_sql} \
         ORDER BY ar.name COLLATE NOCASE, al.name COLLATE NOCASE, t.disc, t.track_no, t.title COLLATE NOCASE"
    );
    if let Some(limit) = track_limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(rusqlite::params_from_iter(values), row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// 实时匹配数（侧边栏徽标/视图头部计数用）
fn count_tracks(conn: &Connection, rules: &[SmartRule]) -> AppResult<i32> {
    let (where_sql, values) = where_clause(rules)?;
    let sql = format!(
        "SELECT COUNT(*) FROM tracks t \
         LEFT JOIN (SELECT track_id, COUNT(*) AS play_count, MAX(played_at) AS last_played \
                    FROM play_history GROUP BY track_id) h ON h.track_id = t.id \
         WHERE {where_sql}"
    );
    let n: i64 = conn.query_row(&sql, rusqlite::params_from_iter(values), |r| r.get(0))?;
    Ok(n as i32)
}

fn parse_rules(json: &str) -> AppResult<Vec<SmartRule>> {
    serde_json::from_str(json).map_err(|e| AppError::Message(format!("规则数据损坏: {e}")))
}

fn list(state: &AppState) -> AppResult<Vec<SmartPlaylist>> {
    let conn = state.conn.lock();
    let mut stmt = conn.prepare("SELECT id, name, rules, track_limit FROM smart_playlists ORDER BY id")?;
    let mut out = Vec::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?,
        ))
    })?;
    for row in rows {
        let (id, name, rules_json, track_limit) = row?;
        let rules = parse_rules(&rules_json)?;
        // 规则损坏的智能歌单按 0 首展示，不拖垮整个列表
        let track_count = count_tracks(&conn, &rules).unwrap_or(0);
        out.push(SmartPlaylist {
            id: id as i32,
            name,
            rules,
            track_limit: track_limit.map(|v| v as i32),
            track_count,
        });
    }
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn smart_playlist_list(state: State<'_, AppState>) -> Result<Vec<SmartPlaylist>, AppErrorDto> {
    list(&state).map_err(AppErrorDto::from)
}

fn tracks(state: &AppState, id: i32) -> AppResult<Vec<Track>> {
    let conn = state.conn.lock();
    let (rules_json, track_limit): (String, Option<i64>) = conn.query_row(
        "SELECT rules, track_limit FROM smart_playlists WHERE id = ?1",
        params![i64::from(id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let rules = parse_rules(&rules_json)?;
    query_tracks(&conn, &rules, track_limit.map(|v| v as i32))
}

#[tauri::command]
#[specta::specta]
pub async fn smart_playlist_tracks(
    state: State<'_, AppState>,
    id: i32,
) -> Result<Vec<Track>, AppErrorDto> {
    tracks(&state, id).map_err(AppErrorDto::from)
}

fn create(state: &AppState, name: String, rules: Vec<SmartRule>, track_limit: Option<i32>) -> AppResult<SmartPlaylist> {
    let name = clean_name(&name)?;
    let limit = clean_limit(track_limit)?;
    where_clause(&rules)?; // 规则合法性前置校验
    let mut conn = state.conn.lock();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO smart_playlists(name, rules, track_limit) VALUES (?1, ?2, ?3)",
        params![name, serde_json::to_string(&rules)?, limit],
    )?;
    let id: i64 = tx.query_row("SELECT last_insert_rowid()", [], |r| r.get(0))?;
    tx.commit()?;
    Ok(SmartPlaylist { id: id as i32, name, rules, track_limit: limit.map(|v| v as i32), track_count: 0 })
}

#[tauri::command]
#[specta::specta]
pub async fn smart_playlist_create(
    state: State<'_, AppState>,
    name: String,
    rules: Vec<SmartRule>,
    track_limit: Option<i32>,
) -> Result<SmartPlaylist, AppErrorDto> {
    create(&state, name, rules, track_limit).map_err(AppErrorDto::from)
}

fn update(state: &AppState, id: i32, name: String, rules: Vec<SmartRule>, track_limit: Option<i32>) -> AppResult<()> {
    let name = clean_name(&name)?;
    let limit = clean_limit(track_limit)?;
    where_clause(&rules)?;
    let mut conn = state.conn.lock();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE smart_playlists SET name = ?1, rules = ?2, track_limit = ?3 WHERE id = ?4",
        params![name, serde_json::to_string(&rules)?, limit, i64::from(id)],
    )?;
    if n == 0 {
        return Err(AppError::Message("智能歌单不存在".into()));
    }
    tx.commit()?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn smart_playlist_update(
    state: State<'_, AppState>,
    id: i32,
    name: String,
    rules: Vec<SmartRule>,
    track_limit: Option<i32>,
) -> Result<(), AppErrorDto> {
    update(&state, id, name, rules, track_limit).map_err(AppErrorDto::from)
}

fn delete(state: &AppState, id: i32) -> AppResult<()> {
    let conn = state.conn.lock();
    conn.execute("DELETE FROM smart_playlists WHERE id = ?1", params![i64::from(id)])?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn smart_playlist_delete(state: State<'_, AppState>, id: i32) -> Result<(), AppErrorDto> {
    delete(&state, id).map_err(AppErrorDto::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn rule(field: SmartField, op: SmartOp, num: Option<f64>, text: Option<&str>) -> SmartRule {
        SmartRule { field, op, num, text: text.map(str::to_string) }
    }

    fn seed(conn: &Connection) {
        conn.execute_batch(
            "INSERT INTO folders(path) VALUES ('X');
             INSERT INTO artists(id, name) VALUES (1, 'Yuki');
             INSERT INTO albums(id, name, artist) VALUES (1, 'Best', 'Yuki');
             -- 最近 N 天规则以 datetime('now') 为基准：种子日期用相对值，
             -- 避免固定日期随真实时间流逝变成定时炸弹
             INSERT INTO tracks(id, path, folder_id, title, artist_id, album_id, genre, year, created_at) VALUES
               (1, 'a.mp3', 1, 'A', 1, 1, 'J-Pop', 2023, datetime('now', '-10 days')),
               (2, 'b.mp3', 1, 'B', 1, 1, 'Rock',  2020, datetime('now', '-20 days')),
               (3, 'c.mp3', 1, 'C', 1, 1, 'J-Pop', 1999, '2026-01-01 00:00:00'),
               (4, 'd.mp3', 1, 'D', 1, 1, NULL,    NULL, '2026-01-01 00:00:00');
             INSERT INTO play_history(track_id, played_at) VALUES
               (1, datetime('now', '-2 days')), (1, datetime('now', '-1 days')),
               (2, '2026-06-01 10:00:00');",
        )
        .unwrap();
    }

    fn titles(conn: &Connection, rules: &[SmartRule], limit: Option<i32>) -> Vec<String> {
        query_tracks(conn, rules, limit)
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect()
    }

    #[test]
    fn 播放次数规则含未播放曲目() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        // 播放 ≥ 2 次：只有 A（未播放的 C/D 按 0 计入，不满足）
        assert_eq!(
            titles(&conn, &[rule(SmartField::PlayCount, SmartOp::Gte, Some(2.0), None)], None),
            vec!["A"]
        );
        // 播放 ≤ 0 次：C、D（B 播过 1 次）
        assert_eq!(
            titles(&conn, &[rule(SmartField::PlayCount, SmartOp::Lte, Some(0.0), None)], None),
            vec!["C", "D"]
        );
    }

    #[test]
    fn 流派规则等于与包含() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        assert_eq!(
            titles(&conn, &[rule(SmartField::Genre, SmartOp::Eq, None, Some("j-pop"))], None),
            vec!["A", "C"]
        );
        assert_eq!(
            titles(&conn, &[rule(SmartField::Genre, SmartOp::Contains, None, Some("-Po"))], None),
            vec!["A", "C"]
        );
        // 通配符按字面匹配：流派里没有真正的 % 字符
        assert!(titles(&conn, &[rule(SmartField::Genre, SmartOp::Contains, None, Some("%"))], None).is_empty());
    }

    #[test]
    fn 年份与添加时间规则() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        assert_eq!(
            titles(&conn, &[rule(SmartField::Year, SmartOp::Gte, Some(2020.0), None)], None),
            vec!["A", "B"]
        );
        // 最近 31 天添加：A（10 天前）、B（20 天前）都命中
        let added = &[rule(SmartField::AddedAt, SmartOp::WithinDays, Some(31.0), None)];
        assert_eq!(titles(&conn, added, None), vec!["A", "B"]);
        // 最近播放 30 天内：只有 A（B 上次播放是固定旧日期 6 月）
        let last = &[rule(SmartField::LastPlayed, SmartOp::WithinDays, Some(30.0), None)];
        assert_eq!(titles(&conn, last, None), vec!["A"]);
    }

    #[test]
    fn 多条件取交集且数量上限生效() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let rules = &[
            rule(SmartField::Genre, SmartOp::Contains, None, Some("J")),
            rule(SmartField::Year, SmartOp::Gte, Some(2000.0), None),
        ];
        assert_eq!(titles(&conn, rules, None), vec!["A"]);
        // 上限截断
        let none: Vec<SmartRule> = vec![];
        assert_eq!(titles(&conn, &none, Some(2)), vec!["A", "B"]);
    }

    #[test]
    fn 非法规则组合与取值报错() {
        // Genre + Gte 非法
        let bad = rule(SmartField::Genre, SmartOp::Gte, Some(1.0), None);
        assert!(rule_condition(&bad).is_err());
        // 缺文本
        let missing = rule(SmartField::Genre, SmartOp::Eq, None, None);
        assert!(rule_condition(&missing).is_err());
        // 负数
        let negative = rule(SmartField::PlayCount, SmartOp::Gte, Some(-1.0), None);
        assert!(rule_condition(&negative).is_err());
        // 上限非法
        assert!(clean_limit(Some(0)).is_err());
        assert!(clean_limit(Some(1001)).is_err());
        assert_eq!(clean_limit(None).unwrap(), None);
    }
}
