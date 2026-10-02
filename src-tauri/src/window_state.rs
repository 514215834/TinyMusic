//! 主窗口尺寸位置记忆（M7，技术设计文档 §5 window_state）：
//! 关闭时保存（物理坐标 + 最大化标记），启动恢复并钳制进显示器边界；
//! 主窗口在 tauri.conf 中默认隐藏（visible=false），恢复完成后再显示，避免位置跳变。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

use crate::overlay::clamp_into_monitor;
use crate::AppState;

const SETTINGS_KEY: &str = "window";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct WindowState {
    /// 物理坐标（与显示器/迷你窗持久化口径一致）
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

fn load(state: &AppState) -> Option<WindowState> {
    let conn = state.conn.lock();
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [SETTINGS_KEY], |r| {
        let raw: String = r.get(0)?;
        Ok(serde_json::from_str::<WindowState>(&raw).ok().filter(|w| w.width > 0 && w.height > 0))
    })
    .ok()
    .flatten()
}

fn persist(state: &AppState, value: &WindowState) {
    let raw = serde_json::to_string(value).unwrap_or_default();
    let conn = state.conn.lock();
    let _ = conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        [SETTINGS_KEY, &raw],
    );
}

/// 立即保存主窗口当前状态（关闭前 / 托盘退出前调用）
pub fn save_now(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let maximized = window.is_maximized().unwrap_or(false);
    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
        let state = app.state::<AppState>();
        persist(
            &state,
            &WindowState {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
                maximized,
            },
        );
    }
}

/// 启动恢复：尺寸 → 位置 → 显示器钳制；持久化为最大化时直接最大化（默认隐藏的窗口随后由 setup 显示）
pub fn restore(app: &AppHandle) {
    let saved = {
        let state = app.state::<AppState>();
        load(&state)
    };
    let Some(window) = app.get_webview_window("main") else { return };
    let Some(saved) = saved else { return };
    if saved.maximized {
        let _ = window.maximize();
        return;
    }
    let _ = window.set_size(PhysicalSize::new(saved.width, saved.height));
    let _ = window.set_position(PhysicalPosition::new(saved.x, saved.y));
    clamp_into_monitor(&window);
}
