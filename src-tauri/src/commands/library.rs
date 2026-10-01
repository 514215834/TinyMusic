use tauri::AppHandle;

use crate::error::AppErrorDto;
use crate::library;

#[tauri::command]
#[specta::specta]
pub async fn library_rescan(app: AppHandle) -> Result<(), AppErrorDto> {
    library::spawn_scan(app);
    Ok(())
}
