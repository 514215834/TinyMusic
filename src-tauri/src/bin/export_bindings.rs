//! 无窗口导出 TS 绑定（前端开发时快速刷新 src/bindings.ts）：
//! `cargo run --bin export_bindings`

fn main() {
    #[cfg(debug_assertions)]
    tinymusic_lib::gui::export_bindings();
}
