//! 播放控制全局快捷键（M7）：播放/暂停、上一首、下一首、显示迷你窗。
//! 换绑复用 overlay_set_shortcut 的模式：先注册新键、成功后才解绑旧键并落库，
//! 失败保持旧键可用；清除绑定立即生效。
//! 播放控制与托盘菜单走同一 tray:action 回流通道（播放引擎在主窗口前端）。

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::error::AppError;
use crate::AppState;

pub const ACTION_TOGGLE: &str = "toggle";
pub const ACTION_PREV: &str = "prev";
pub const ACTION_NEXT: &str = "next";
pub const ACTION_MINI: &str = "mini";

/// 设置页可绑定的全部动作（顺序即设置页展示顺序）
pub const ACTIONS: &[&str] = &[ACTION_TOGGLE, ACTION_PREV, ACTION_NEXT, ACTION_MINI];

/// settings 表 "shortcuts" 键的载荷；None = 未绑定（默认全部未绑定，由用户按需开启）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutSettings {
    pub toggle: Option<String>,
    pub prev: Option<String>,
    pub next: Option<String>,
    pub mini: Option<String>,
}

/// 当前已注册的动作快捷键（规范化输入串），用于换绑时解绑旧键
static REGISTERED: LazyLock<Mutex<HashMap<&'static str, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn load_settings(state: &AppState) -> ShortcutSettings {
    let conn = state.conn.lock();
    conn.query_row("SELECT value FROM settings WHERE key = 'shortcuts'", [], |r| {
        let raw: String = r.get(0)?;
        Ok(serde_json::from_str::<ShortcutSettings>(&raw).unwrap_or_default())
    })
    .unwrap_or_default()
}

fn save_settings(state: &AppState, s: &ShortcutSettings) {
    let conn = state.conn.lock();
    let raw = serde_json::to_string(s).unwrap_or_default();
    let _ = conn.execute(
        "INSERT INTO settings(key, value) VALUES ('shortcuts', ?1) \
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        [raw],
    );
}

fn field_of(s: &ShortcutSettings, action: &str) -> Option<String> {
    match action {
        ACTION_TOGGLE => s.toggle.clone(),
        ACTION_PREV => s.prev.clone(),
        ACTION_NEXT => s.next.clone(),
        ACTION_MINI => s.mini.clone(),
        _ => None,
    }
}

fn set_field(s: &mut ShortcutSettings, action: &str, value: Option<String>) {
    match action {
        ACTION_TOGGLE => s.toggle = value,
        ACTION_PREV => s.prev = value,
        ACTION_NEXT => s.next = value,
        ACTION_MINI => s.mini = value,
        _ => {}
    }
}

fn on_press(app: &AppHandle, action: &str) {
    if action == ACTION_MINI {
        crate::overlay::toggle(app);
        return;
    }
    // 播放控制经托盘同款事件回流主窗口 player store 执行
    let _ = app.emit("tray:action", action);
}

/// 注册动作快捷键；失败返回原因（占用/格式无效），不产生任何副作用
fn register(app: &AppHandle, action: &'static str, shortcut: &str) -> Result<(), String> {
    let parsed: Shortcut = shortcut
        .trim()
        .parse()
        .map_err(|e| format!("无法解析快捷键「{shortcut}」: {e}"))?;
    let action_owned = action;
    app.global_shortcut()
        .on_shortcut(parsed, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                on_press(app, action_owned);
            }
        })
        .map_err(|e| format!("快捷键注册失败（可能已被其他程序占用）: {e}"))?;
    REGISTERED.lock().insert(action, shortcut.trim().to_string());
    Ok(())
}

/// 解除某动作当前已注册的快捷键（存在才解）
fn unregister_action(app: &AppHandle, action: &str) {
    if let Some(s) = REGISTERED.lock().remove(action) {
        if let Ok(parsed) = s.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(parsed);
        }
    }
}

/// 启动 / 备份恢复后：解绑全部动作快捷键并按持久化配置重注册；失败仅告警不阻断
pub fn apply_all(app: &AppHandle) {
    for action in ACTIONS {
        unregister_action(app, action);
    }
    let settings = {
        let state = app.state::<AppState>();
        load_settings(&state)
    };
    for action in ACTIONS {
        if let Some(shortcut) = field_of(&settings, action) {
            if let Err(e) = register(app, action, &shortcut) {
                eprintln!("[shortcut] 动作「{action}」快捷键「{shortcut}」注册失败: {e}");
            }
        }
    }
}

/// 运行时设置/清除某动作的快捷键（设置页调用）：注册成功才落库，失败保持旧键
pub fn set(app: &AppHandle, action: &str, shortcut: Option<&str>) -> crate::error::AppResult<()> {
    let Some(&action_key) = ACTIONS.iter().find(|a| **a == action) else {
        return Err(AppError::Message(format!("未知动作: {action}")));
    };
    let state = app.state::<AppState>();
    let mut settings = load_settings(&state);

    match shortcut.map(str::trim).filter(|s| !s.is_empty()) {
        // 清除绑定
        None => {
            unregister_action(app, action_key);
            set_field(&mut settings, action_key, None);
            save_settings(&state, &settings);
            Ok(())
        }
        Some(s) => {
            // 校验格式（不注册）
            if let Err(e) = s.parse::<Shortcut>() {
                return Err(AppError::Message(format!("无法解析快捷键「{s}」: {e}")));
            }
            let old = field_of(&settings, action_key);
            if old.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(s)) {
                if REGISTERED.lock().contains_key(action_key) {
                    return Ok(());
                }
                // 已持久化但未注册成功（如启动时被占用）：重试注册即可
                return register(app, action_key, s).map_err(AppError::Message);
            }
            // 本应用其他功能（其他动作/迷你窗换绑键）已占用同一组合时直接拒绝
            if app.global_shortcut().is_registered(s.parse::<Shortcut>().unwrap()) {
                return Err(AppError::Message(format!(
                    "快捷键「{s}」已被占用（可能已绑定本应用其他功能）"
                )));
            }
            // 先注册新键，成功后才解绑旧键并落库——失败路径上旧键始终可用
            register(app, action_key, s).map_err(AppError::Message)?;
            if let Some(old_s) = old {
                if !old_s.eq_ignore_ascii_case(s) {
                    if let Ok(old_parsed) = old_s.parse::<Shortcut>() {
                        let _ = app.global_shortcut().unregister(old_parsed);
                    }
                }
            }
            set_field(&mut settings, action_key, Some(s.to_string()));
            save_settings(&state, &settings);
            Ok(())
        }
    }
}
