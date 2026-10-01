use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tauri::AppHandle;

/// 事件静默期：文件批量改动（下载、移动）期间不重复触发
const QUIET_MS: u64 = 1500;

/// 目录监听句柄：folder_add/folder_remove 时动态增删监听目录
pub struct WatchHandle {
    watcher: Mutex<RecommendedWatcher>,
}

impl WatchHandle {
    pub fn watch(&self, path: &Path) {
        if let Err(e) = self.watcher.lock().watch(path, RecursiveMode::Recursive) {
            eprintln!("[watch] 监听目录失败 {path:?}: {e}");
        }
    }

    pub fn unwatch(&self, path: &Path) {
        if let Err(e) = self.watcher.lock().unwatch(path) {
            eprintln!("[watch] 取消监听失败 {path:?}: {e}");
        }
    }
}

/// 启动目录监听：任一被监听目录内文件变更后，经静默期去抖触发一次增量扫描，
/// 由 scan:done 事件驱动前端刷新（曲库 5s 内自动同步的验收路径）。
pub fn start(app: AppHandle) -> Result<WatchHandle, notify::Error> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if res.is_ok() {
            // 事件内容不重要（增量扫描自带 mtime/size 判断），只需要"有变更"信号
            let _ = tx.send(());
        }
    })?;

    std::thread::spawn(move || loop {
        match rx.recv() {
            Ok(()) => {
                // 持续吸收事件直到静默期结束，再触发一次扫描
                loop {
                    match rx.recv_timeout(Duration::from_millis(QUIET_MS)) {
                        Ok(()) => continue,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                crate::library::spawn_scan(app.clone());
            }
            Err(_) => return,
        }
    });

    Ok(WatchHandle { watcher: Mutex::new(watcher) })
}
