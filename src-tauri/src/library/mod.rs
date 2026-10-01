pub(crate) mod covers;
mod metadata;
pub mod m3u8;
pub mod scanner;
pub mod watcher;

use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::error::AppResult;
use crate::models::{ScanDone, ScanProgress};
use crate::AppState;

/// 在独立线程执行扫描（不阻塞 UI/命令），进度经 scan:progress / scan:done 事件推送；
/// 全部文件解析失败时追加 scan:error 提示
pub fn spawn_scan(app: AppHandle) {
    std::thread::spawn(move || {
        match run_scan(&app) {
            Ok(done) => {
                // scan:done 是前端刷新曲库列表的唯一信号，成功也必须发出
                let _ = app.emit("scan:done", &done);
                if done.added + done.updated == 0 && done.errors > 0 {
                    let _ = app.emit(
                        "scan:error",
                        format!("扫描完成但有 {} 个文件解析失败，请检查文件格式", done.errors),
                    );
                }
            }
            Err(e) => {
                let _ = app.emit("scan:error", e.to_string());
            }
        }
    });
}

fn run_scan(app: &AppHandle) -> AppResult<ScanDone> {
    let started = std::time::Instant::now();
    let state = app.state::<AppState>();
    let db_path = state.db_path.clone();
    let covers_dir = state.covers_dir.clone();
    let _ = state;

    let mut conn = db::open(&db_path)?;
    let folders = db::folder_rows(&conn)?;

    let outcome = scanner::scan_folders(&mut conn, &folders, &covers_dir, |done, total| {
        let _ = app.emit("scan:progress", ScanProgress { done, total });
    })?;

    Ok(ScanDone {
        added: outcome.added,
        updated: outcome.updated,
        removed: outcome.removed,
        unchanged: outcome.unchanged,
        errors: outcome.errors,
        duration_ms: started.elapsed().as_millis() as u32,
    })
}
