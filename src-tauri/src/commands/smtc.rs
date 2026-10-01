use tauri::AppHandle;

use crate::error::AppErrorDto;
use crate::models::SmtcState;

/// 主窗口 → Rust：转发播放状态给系统媒体浮层（SMTC）；
/// 非 Windows 平台为 no-op（本项目 Windows 优先，规范见技术设计文档 §6）
#[tauri::command]
#[specta::specta]
pub async fn smtc_update(app: AppHandle, state: SmtcState) -> Result<(), AppErrorDto> {
    #[cfg(windows)]
    crate::smtc::update(app, state);
    #[cfg(not(windows))]
    let _ = (app, state);
    Ok(())
}
