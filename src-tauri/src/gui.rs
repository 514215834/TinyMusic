//! GUI 启动层：tauri Builder 装配（单测构建不编译本模块，
//! 避免 test exe 链入 tao/wry 的 comctl32 v6 依赖）。

use parking_lot::Mutex;
use tauri::Manager;

use crate::db;
use crate::AppState;

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        crate::commands::albums::album_tracks,
        crate::commands::albums::albums_query,
        crate::commands::albums::artist_tracks,
        crate::commands::albums::artists_query,
        crate::commands::backup::backup_export,
        crate::commands::backup::backup_restore,
        crate::commands::covers::cover_path,
        crate::commands::duplicates::duplicate_resolve,
        crate::commands::duplicates::duplicates_scan,
        crate::commands::favorites::favorite_toggle,
        crate::commands::favorites::favorites_ids,
        crate::commands::favorites::favorites_list,
        crate::commands::folders::drop_import,
        crate::commands::folders::folder_add,
        crate::commands::folders::folder_list,
        crate::commands::folders::folder_remove,
        crate::commands::folders::folder_tracks,
        crate::commands::history::history_add,
        crate::commands::library::library_rescan,
        crate::commands::lyrics::lyrics_get,
        crate::commands::overlay::overlay_hide,
        crate::commands::overlay::overlay_set_locked,
        crate::commands::overlay::overlay_set_opacity,
        crate::commands::overlay::overlay_set_shortcut,
        crate::commands::overlay::overlay_toggle,
        crate::commands::playlists::playlist_add_tracks,
        crate::commands::playlists::playlist_create,
        crate::commands::playlists::playlist_delete,
        crate::commands::playlists::playlist_export,
        crate::commands::playlists::playlist_import,
        crate::commands::playlists::playlist_list,
        crate::commands::playlists::playlist_remove_track,
        crate::commands::playlists::playlist_rename,
        crate::commands::playlists::playlist_reorder,
        crate::commands::playlists::playlist_tracks,
        crate::commands::scrape::scrape_album,
        crate::commands::scrape::scrape_apply_album,
        crate::commands::scrape::scrape_apply_lyrics,
        crate::commands::scrape::scrape_apply_track,
        crate::commands::scrape::scrape_lyrics,
        crate::commands::scrape::scrape_track,
        crate::commands::search::search_tracks,
        crate::commands::settings::settings_get,
        crate::commands::settings::settings_set,
        crate::commands::shortcuts::shortcut_set,
        crate::commands::smart_playlists::smart_playlist_create,
        crate::commands::smart_playlists::smart_playlist_delete,
        crate::commands::smart_playlists::smart_playlist_list,
        crate::commands::smart_playlists::smart_playlist_tracks,
        crate::commands::smart_playlists::smart_playlist_update,
        crate::commands::smtc::smtc_update,
        crate::commands::stats::history_recent,
        crate::commands::stats::history_top,
        crate::commands::tags::tag_update,
        crate::commands::tags::tag_update_batch,
        crate::commands::tracks::track_get,
        crate::commands::tracks::tracks_all,
        crate::commands::tracks::tracks_query,
    ])
}

/// debug 构建下导出 TS 绑定（应用启动时自动执行；
/// bin/export_bindings.rs 无窗口环境下单独调用）
#[cfg(debug_assertions)]
pub fn export_bindings() {
    specta_builder()
        .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
        .expect("导出 TS 绑定失败");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    export_bindings();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(specta_builder().invoke_handler())
        .setup(move |app| {
            specta_builder().mount_events(app);
            let handle = app.handle().clone();

            // 系统托盘（§4.2）
            crate::tray::setup(&handle)?;

            let data_dir = app.path().app_data_dir()?;
            let db_path = data_dir.join("db.sqlite3");
            let covers_dir = data_dir.join("covers");
            std::fs::create_dir_all(&covers_dir)?;

            let conn = db::open(&db_path)?;

            // 增量监听：曲库目录文件增删改 → 去抖 → 自动增量扫描
            let watcher = crate::library::watcher::start(handle.clone())?;

            // asset 协议范围：封面缓存目录 + 已有曲库目录（重启后依然可访问）
            let scope = app.asset_protocol_scope();
            scope.allow_directory(&covers_dir, false)?;
            for (_, dir) in db::folder_rows(&conn)? {
                scope.allow_directory(std::path::Path::new(&dir), true)?;
                watcher.watch(std::path::Path::new(&dir));
            }

            app.manage(AppState {
                conn: Mutex::new(conn),
                db_path,
                covers_dir,
                watcher,
            });

            // 迷你播放器悬停检测/回锁 watcher + 可配置全局快捷键（§6.2，读 settings，需 AppState 就绪）
            crate::overlay::spawn_overlay_watcher(handle.clone());
            crate::overlay::register_shortcut_from_settings(&handle);
            // 播放控制类全局快捷键（M7，默认全部未绑定）
            crate::shortcuts::apply_all(&handle);

            // 主窗口状态恢复（M7）：窗口在 conf 中默认隐藏，恢复尺寸位置后再显示，避免跳变
            crate::window_state::restore(&handle);
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
            }

            // SMTC 系统媒体浮层（媒体键经 smtc:command 回流，§2.3）
            #[cfg(windows)]
            if let Some(main) = app.get_webview_window("main") {
                crate::smtc::init(&main, handle.clone());
            }

            // 主窗口关闭 = 退出应用（迷你悬浮窗不拦截退出）；关闭前保存窗口状态（M7）
            let handle_for_exit = handle.clone();
            if let Some(main) = app.get_webview_window("main") {
                main.on_window_event(move |event| match event {
                    tauri::WindowEvent::CloseRequested { .. } => {
                        crate::window_state::save_now(&handle_for_exit);
                    }
                    tauri::WindowEvent::Destroyed => handle_for_exit.exit(0),
                    _ => {}
                });
            }

            // 启动即增量扫描（mtime/size 未变的文件自动跳过）
            crate::library::spawn_scan(handle.clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
