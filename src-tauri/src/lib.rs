pub mod commands;
pub mod db;
pub mod error;
pub mod library;
pub mod models;
pub mod overlay;
#[cfg(windows)]
pub mod smtc;
pub mod shortcuts;
pub mod tray;
pub mod window_state;

use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::Connection;

use crate::library::watcher::WatchHandle;

/// 全局共享状态：主连接（命令用）+ 扫描线程用的路径 + 目录监听句柄
pub struct AppState {
    pub conn: Mutex<Connection>,
    pub db_path: PathBuf,
    pub covers_dir: PathBuf,
    pub watcher: WatchHandle,
}

// GUI 启动层仅在非单测构建编译（见 gui.rs 顶部说明）
#[cfg(not(test))]
pub mod gui;
