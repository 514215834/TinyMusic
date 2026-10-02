use tauri::AppHandle;

use crate::error::AppErrorDto;
use crate::shortcuts;

/// 设置/清除播放控制类全局快捷键（M7，设置页调用）：
/// shortcut 为 None 或空串 = 清除绑定；注册失败返回错误并保持旧键可用
#[tauri::command]
#[specta::specta]
pub async fn shortcut_set(
    app: AppHandle,
    action: String,
    shortcut: Option<String>,
) -> Result<(), AppErrorDto> {
    shortcuts::set(&app, &action, shortcut.as_deref()).map_err(AppErrorDto::from)
}
