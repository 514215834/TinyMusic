use tauri::AppHandle;

use crate::error::{AppError, AppErrorDto};
use crate::overlay;

/// 显示/隐藏迷你悬浮窗（托盘与主窗口播放条共用入口）
#[tauri::command]
#[specta::specta]
pub async fn overlay_toggle(app: AppHandle) -> Result<(), AppErrorDto> {
    overlay::toggle(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn overlay_hide(app: AppHandle) -> Result<(), AppErrorDto> {
    overlay::hide(&app).map_err(AppError::from).map_err(AppErrorDto::from)
}

/// 锁定⇄调整态（锁定 = 鼠标完全穿透）
#[tauri::command]
#[specta::specta]
pub async fn overlay_set_locked(app: AppHandle, locked: bool) -> Result<(), AppErrorDto> {
    overlay::set_locked(&app, locked);
    Ok(())
}

/// 透明度调节 0.3–0.9，即时生效并持久化
#[tauri::command]
#[specta::specta]
pub async fn overlay_set_opacity(app: AppHandle, opacity: f64) -> Result<(), AppErrorDto> {
    overlay::set_opacity(&app, opacity);
    Ok(())
}

/// 配置调整/锁定切换的全局快捷键（设置页调用，即时生效并持久化）
#[tauri::command]
#[specta::specta]
pub async fn overlay_set_shortcut(app: AppHandle, shortcut: String) -> Result<(), AppErrorDto> {
    overlay::set_shortcut(&app, &shortcut).map_err(AppErrorDto::from)
}
