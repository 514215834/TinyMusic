//! 系统托盘（功能设计文档 §4.2）：迷你播放器开关/调整、播放控制、退出。
//! 播放控制经 tray:action 事件回流主窗口执行（技术设计文档 §2.3）。

use std::sync::OnceLock;

use tauri::menu::{CheckMenuItem, MenuBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::overlay;

static MINI_CHECK: OnceLock<CheckMenuItem<Wry>> = OnceLock::new();

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show_mini =
        CheckMenuItem::with_id(app, "mini_show", "显示迷你播放器", true, false, None::<&str>)?;
    let adjust_mini =
        tauri::menu::MenuItem::with_id(app, "mini_adjust", "调整迷你播放器", true, None::<&str>)?;
    let play_pause =
        tauri::menu::MenuItem::with_id(app, "play_pause", "播放 / 暂停", true, None::<&str>)?;
    let prev = tauri::menu::MenuItem::with_id(app, "prev", "上一首", true, None::<&str>)?;
    let next = tauri::menu::MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?;
    let quit =
        tauri::menu::MenuItem::with_id(app, "quit", "退出 TinyMusic", true, None::<&str>)?;
    let _ = MINI_CHECK.set(show_mini.clone());

    let menu = MenuBuilder::new(app)
        .item(&show_mini)
        .item(&adjust_mini)
        .separator()
        .item(&play_pause)
        .item(&prev)
        .item(&next)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::with_id("tray")
        .icon(app.default_window_icon().expect("应用图标").clone())
        .tooltip("TinyMusic")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            // 左键单击托盘唤起主窗口
            if let TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, button_state: tauri::tray::MouseButtonState::Up, .. } = event {
                if let Some(main) = tray.app_handle().get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.unminimize();
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "mini_show" => {
            // CheckMenuItem 在事件派发前已翻转勾选态
            let checked = MINI_CHECK.get().is_some_and(|item| item.is_checked().unwrap_or(false));
            if checked {
                let _ = overlay::show(app);
            } else {
                let _ = overlay::hide(app);
            }
        }
        "mini_adjust" => overlay::set_locked(app, false),
        "play_pause" => emit_action(app, "toggle"),
        "prev" => emit_action(app, "prev"),
        "next" => emit_action(app, "next"),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// 迷你窗开关时同步托盘勾选态（overlay 模块调用）
pub fn set_mini_checked(_app: &AppHandle, checked: bool) {
    if let Some(item) = MINI_CHECK.get() {
        let _ = item.set_checked(checked);
    }
}

fn emit_action(app: &AppHandle, action: &str) {
    let _ = app.emit("tray:action", action);
}
