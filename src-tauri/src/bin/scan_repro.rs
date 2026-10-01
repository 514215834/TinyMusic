//! headless 扫描调试工具（仅 debug 构建）：不经 GUI 直接驱动与 App 相同的扫描管线。
//!
//! 用法（在 src-tauri 下）：
//!   cargo run --bin scan_repro                     # 扫描曲库里已登记的全部文件夹
//!   cargo run --bin scan_repro -- G:\Music\新目录   # 先登记再扫描指定文件夹
//!
//! 结束后打印 ScanOutcome 统计与曲目样例，用于验证"扫描→入库"链路。

#[cfg(debug_assertions)]
fn main() {
    use std::path::PathBuf;

    use tinymusic_lib::{db, library::scanner};

    let mut args: Vec<String> = std::env::args().skip(1).collect();

    let app_data = std::env::var("APPDATA").expect("APPDATA 环境变量未设置");
    let data_dir = PathBuf::from(app_data).join("com.tinymusic.app");
    let db_path = data_dir.join("db.sqlite3");
    let covers_dir = data_dir.join("covers");
    println!("db: {}", db_path.display());

    let mut conn = db::open(&db_path).expect("打开数据库失败");

    // 命令行传入的文件夹先登记（与 folder_add 同语义：普通路径入库）
    for path in &args {
        let p = PathBuf::from(path);
        assert!(p.is_dir(), "目录不存在: {path}");
        conn.execute("INSERT OR IGNORE INTO folders(path) VALUES (?1)", [path])
            .expect("登记文件夹失败");
    }
    args.clear();

    let folders = db::folder_rows(&conn).expect("读取文件夹失败");
    println!("folders: {:?}", folders);

    let outcome = scanner::scan_folders(&mut conn, &folders, &covers_dir, |done, total| {
        if total > 0 && done % 100 == 0 {
            eprintln!("progress {done}/{total}");
        }
    })
    .expect("扫描失败");

    println!("\noutcome: {outcome:?}");

    let total: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0)).unwrap();
    println!("tracks total: {total}");
    let mut stmt = conn.prepare("SELECT id, title, artist_id, album_id, duration_sec FROM tracks LIMIT 5").unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
            ))
        })
        .unwrap();
    for r in rows {
        println!("sample: {:?}", r.unwrap());
    }
}

#[cfg(not(debug_assertions))]
fn main() {
    eprintln!("scan_repro 仅在 debug 构建可用（cargo run --bin scan_repro）");
    std::process::exit(1);
}
