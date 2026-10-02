//! 迷你悬浮播放器（技术设计文档 §6）：
//! 置顶透明无边框窗口；锁定态鼠标完全穿透；调整态可拖动、10s 无操作自动回锁；
//! 位置与透明度持久化在 settings 表 "overlay" 键。

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::tray;
use crate::AppState;

const LABEL: &str = "mini-overlay";
const WIDTH: f64 = 352.0;
const HEIGHT: f64 = 198.0;
const MARGIN: f64 = 16.0;
const RELOCK_SECS: u64 = 10;
/// 调整/锁定切换的默认全局快捷键（可在设置页改）
const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+M";

static LOCKED: AtomicBool = AtomicBool::new(true);
/// 调整态最近一次交互（拖动）时间，watchdog 据此自动回锁
static LAST_ACTIVITY_MS: AtomicI64 = AtomicI64::new(0);
static SAVE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static LAST_POS_X: AtomicI64 = AtomicI64::new(i64::MIN);
static LAST_POS_Y: AtomicI64 = AtomicI64::new(i64::MIN);
/// 光标是否悬停在迷你窗矩形内（锁定态穿透收不到 hover，由 watcher 轮询驱动）
static HOVERED: AtomicBool = AtomicBool::new(false);
/// 当前已注册的全局快捷键（规范化输入串），None = 尚未注册
static REGISTERED_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct OverlaySettings {
    x: Option<f64>,
    y: Option<f64>,
    opacity: f64,
    shortcut: Option<String>,
}
impl Default for OverlaySettings {
    fn default() -> Self {
        Self { x: None, y: None, opacity: 0.6, shortcut: None }
    }
}

fn load_settings(state: &AppState) -> OverlaySettings {
    let conn = state.conn.lock();
    let parsed = conn
        .query_row("SELECT value FROM settings WHERE key = 'overlay'", [], |r| {
            let raw: String = r.get(0)?;
            Ok(serde_json::from_str::<OverlaySettings>(&raw).unwrap_or_default())
        })
        .unwrap_or_default();
    // 旧默认 0.85 在"悬停变透明"的新交互下几乎无感知，静默迁移到新默认 0.6（仅命中一次）
    if (parsed.opacity - 0.85).abs() < f64::EPSILON {
        let migrated = OverlaySettings { opacity: 0.6, ..parsed };
        save_settings(state, &migrated);
        return migrated;
    }
    parsed
}

fn save_settings(state: &AppState, s: &OverlaySettings) {
    let conn = state.conn.lock();
    let raw = serde_json::to_string(s).unwrap_or_default();
    let _ = conn.execute(
        "INSERT INTO settings(key, value) VALUES ('overlay', ?1) \
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        [raw],
    );
}

fn clamp_opacity(op: f64) -> f64 {
    op.clamp(0.3, 0.9)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 窗口当前是否打开（供托盘勾选态使用）
pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}

/// 打开（或置前）迷你窗：不存在时按持久化位置创建，并推送当前锁定态/透明度
pub fn show(app: &AppHandle) -> tauri::Result<()> {
    if let Some(existing) = app.get_webview_window(LABEL) {
        let _ = existing.show();
        push_state(app);
        tray::set_mini_checked(app, true);
        return Ok(());
    }

    let (saved_x, saved_y) = {
        let state = app.state::<AppState>();
        let s = load_settings(&state);
        (s.x, s.y)
    };

    // 默认停靠主屏右上角（§6.1），逻辑坐标；有持久化位置（物理坐标）则构建后还原
    let mut builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("/overlay".into()))
        .title("TinyMusic Mini")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false);
    if saved_x.is_none() {
        if let Ok(Some(monitor)) = app.primary_monitor() {
            let scale = monitor.scale_factor();
            let right = monitor.size().width as f64 / scale - WIDTH - MARGIN;
            builder = builder.position(right.max(0.0), MARGIN);
        }
    }

    let window = builder.build()?;
    #[cfg(target_os = "windows")]
    apply_rounded_corners(&window);
    if let (Some(x), Some(y)) = (saved_x, saved_y) {
        let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
    }
    clamp_into_monitor(&window);

    // 拖动期间持续刷新活动时间与最新位置（去抖落库）
    let app_for_move = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Moved(pos) = event {
            LAST_ACTIVITY_MS.store(now_ms(), Ordering::SeqCst);
            LAST_POS_X.store(pos.x as i64, Ordering::SeqCst);
            LAST_POS_Y.store(pos.y as i64, Ordering::SeqCst);
            schedule_position_save(app_for_move.clone());
        }
    });

    let _ = window.set_ignore_cursor_events(LOCKED.load(Ordering::SeqCst));
    HOVERED.store(false, Ordering::SeqCst);
    push_state(app);
    tray::set_mini_checked(app, true);
    Ok(())
}

/// Win11：让 DWM 按圆角合成窗口（DWMWA_WINDOW_CORNER_PREFERENCE），
/// 圆角缺口像素由系统裁剪；配合前端去掉 CSS 外投影，
/// 浅色桌面上圆角处不再透出灰色残影。Win10 无此属性时静默忽略。
///
/// 另去掉 Win11 给圆角窗口自动描的系统边框（DWMWA_BORDER_COLOR）：
/// 该边框在浅色主题下是白色，且画在窗口边缘（半径 ~8px）——比前端卡片
/// 圆角（12px）更靠外，会在四角透明缺口里悬空露出白色像素。
#[cfg(target_os = "windows")]
fn apply_rounded_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        DWMWA_COLOR_NONE,
    };

    // tauri 与本 crate 共用 windows 0.62，HWND 类型直接统一（同 smtc.rs）
    if let Ok(hwnd) = window.hwnd() {
        let preference = DWMWCP_ROUND;
        let border_none = COLORREF(DWMWA_COLOR_NONE);
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &preference as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&preference) as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &border_none as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&border_none) as u32,
            );
        }
    }
}

/// 恢复持久化位置后夹取到当前显示器可见范围（含 16px 逻辑边距）：
/// 窗口尺寸变更或显示器变更后，旧坐标可能把窗口推出屏幕边缘
/// （例：加宽 10% 后沿用旧 x = 右边贴边/出屏）。
/// 迷你窗与主窗口状态恢复（M7 window_state）共用。
pub(crate) fn clamp_into_monitor(window: &tauri::WebviewWindow) {
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let margin = (MARGIN * monitor.scale_factor()).round() as i32;
    let mpos = monitor.position();
    let msize = monitor.size();
    let min_x = mpos.x + margin;
    let min_y = mpos.y + margin;
    let max_x = (mpos.x + msize.width as i32 - size.width as i32 - margin).max(min_x);
    let max_y = (mpos.y + msize.height as i32 - size.height as i32 - margin).max(min_y);
    let nx = pos.x.clamp(min_x, max_x);
    let ny = pos.y.clamp(min_y, max_y);
    if nx != pos.x || ny != pos.y {
        let _ = window.set_position(tauri::PhysicalPosition::new(nx, ny));
    }
}

/// 关闭并保存位置
pub fn hide(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Ok(pos) = window.outer_position() {
            persist_position(app, pos.x as f64, pos.y as f64);
        }
        window.close()?;
    }
    tray::set_mini_checked(app, false);
    Ok(())
}

pub fn toggle(app: &AppHandle) {
    if is_open(app) {
        let _ = hide(app);
    } else {
        let _ = show(app);
    }
}

pub fn is_locked() -> bool {
    LOCKED.load(Ordering::SeqCst)
}

/// 锁定⇄调整态切换：锁定 = 鼠标完全穿透
pub fn set_locked(app: &AppHandle, locked: bool) {
    LOCKED.store(locked, Ordering::SeqCst);
    if !locked {
        LAST_ACTIVITY_MS.store(now_ms(), Ordering::SeqCst);
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_ignore_cursor_events(locked);
    }
    push_state(app);
}

pub fn toggle_locked(app: &AppHandle) {
    set_locked(app, !is_locked());
}

/// 透明度调节（0.3–0.9），即时生效并持久化
pub fn set_opacity(app: &AppHandle, opacity: f64) {
    let opacity = clamp_opacity(opacity);
    {
        let state = app.state::<AppState>();
        let mut s = load_settings(&state);
        s.opacity = opacity;
        save_settings(&state, &s);
    }
    push_state(app);
}

fn push_state(app: &AppHandle) {
    let opacity = {
        let state = app.state::<AppState>();
        load_settings(&state).opacity
    };
    let _ = app.emit(
        "overlay:state",
        serde_json::json!({
            "locked": is_locked(),
            "hover": HOVERED.load(Ordering::SeqCst),
            "opacity": clamp_opacity(opacity),
        }),
    );
}

fn persist_position(app: &AppHandle, x: f64, y: f64) {
    let state = app.state::<AppState>();
    let mut s = load_settings(&state);
    s.x = Some(x);
    s.y = Some(y);
    save_settings(&state, &s);
}

/// 拖动去抖：800ms 内的 moved 事件只落库一次（后台线程，不阻塞窗口事件）
fn schedule_position_save(app: AppHandle) {
    if SAVE_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        let (x, y) = (LAST_POS_X.load(Ordering::SeqCst), LAST_POS_Y.load(Ordering::SeqCst));
        if x != i64::MIN && y != i64::MIN {
            persist_position(&app, x as f64, y as f64);
        }
        SAVE_IN_FLIGHT.store(false, Ordering::SeqCst);
    });
}

/// 悬停检测 + 调整态自动回锁共用循环（§6.2）：
/// 锁定态窗口鼠标穿透，OS 不投递任何鼠标事件，前端 CSS :hover 失效——
/// 这里轮询光标是否落在窗口矩形内，变化时经 overlay:state 推送 hover 字段，
/// 让锁定态同样驱动"悬停显隐"视觉（仅展示，仍不可操作）。
/// 回锁时机：调整态下悬停在卡片上视为持续操作（不回锁），
/// 光标离开后 10s 无操作才回锁——避免"解锁后还没来得及拖就被锁回"。
pub fn spawn_overlay_watcher(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(150));

        if !is_locked() {
            if HOVERED.load(Ordering::SeqCst) {
                LAST_ACTIVITY_MS.store(now_ms(), Ordering::SeqCst);
            } else if (now_ms() - LAST_ACTIVITY_MS.load(Ordering::SeqCst)) / 1000
                >= RELOCK_SECS as i64
            {
                set_locked(&app, true);
            }
        }

        let Some(window) = app.get_webview_window(LABEL) else {
            continue;
        };
        let hovered = match (window.cursor_position(), window.outer_position(), window.outer_size())
        {
            (Ok(cursor), Ok(pos), Ok(size)) => {
                let right = pos.x as f64 + size.width as f64;
                let bottom = pos.y as f64 + size.height as f64;
                cursor.x >= pos.x as f64
                    && cursor.x < right
                    && cursor.y >= pos.y as f64
                    && cursor.y < bottom
            }
            _ => false,
        };
        if hovered != HOVERED.swap(hovered, Ordering::SeqCst) {
            push_state(&app);
        }
    });
}

/// 注册调整/锁定切换的全局快捷键；失败返回原因（占用/格式无效），由调用方决定是否阻断
fn register_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let parsed: Shortcut = shortcut
        .trim()
        .parse()
        .map_err(|e| format!("无法解析快捷键「{shortcut}」: {e}"))?;
    app.global_shortcut()
        .on_shortcut(parsed, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_locked(app);
            }
        })
        .map_err(|e| format!("快捷键注册失败（可能已被其他程序占用）: {e}"))?;
    *REGISTERED_SHORTCUT.lock() = Some(shortcut.trim().to_string());
    Ok(())
}

/// 启动时按持久化配置注册快捷键（无配置则用默认值）；失败仅告警不阻断启动
pub fn register_shortcut_from_settings(app: &AppHandle) {
    let configured = {
        let state = app.state::<AppState>();
        load_settings(&state).shortcut
    };
    let shortcut = configured.unwrap_or_else(|| DEFAULT_SHORTCUT.to_string());
    if let Err(e) = register_shortcut(app, &shortcut) {
        eprintln!("[shortcut] 全局快捷键「{shortcut}」注册失败: {e}");
    }
}

/// 运行时更换快捷键（设置页调用）：解绑旧键、注册新键并持久化
pub fn set_shortcut(app: &AppHandle, shortcut: &str) -> crate::error::AppResult<()> {
    use crate::error::AppError;

    let s = shortcut.trim();
    if s.is_empty() {
        return Err(AppError::Message("快捷键不能为空".into()));
    }
    // 校验格式（同时用于回滚），不实际注册
    if let Err(e) = s.parse::<Shortcut>() {
        return Err(AppError::Message(format!("无法解析快捷键「{s}」: {e}")));
    }
    if REGISTERED_SHORTCUT.lock().as_deref().is_some_and(|c| c.eq_ignore_ascii_case(s)) {
        return Ok(());
    }
    // 本应用其他功能（播放控制快捷键 M7）已占用同一组合时直接拒绝，
    // 避免 on_shortcut 对已注册键叠加处理器导致双触发
    if let Ok(parsed) = s.parse::<Shortcut>() {
        if app.global_shortcut().is_registered(parsed) {
            return Err(AppError::Message(format!(
                "快捷键「{s}」已被占用（可能已绑定本应用其他功能）"
            )));
        }
    }

    let gs = app.global_shortcut();
    let old = REGISTERED_SHORTCUT.lock().clone();
    register_shortcut(app, s).map_err(AppError::Message)?;
    if let Some(old_s) = old {
        if let Ok(old_parsed) = old_s.parse::<Shortcut>() {
            let _ = gs.unregister(old_parsed);
        }
    }
    {
        let state = app.state::<AppState>();
        let mut settings = load_settings(&state);
        settings.shortcut = Some(s.to_string());
        save_settings(&state, &settings);
    }
    Ok(())
}

/// 备份恢复后按持久化配置重注册快捷键（M7）：配置未变时幂等，失败仅告警
pub fn reapply_shortcut(app: &AppHandle) {
    let configured = {
        let state = app.state::<AppState>();
        load_settings(&state).shortcut
    };
    let Some(s) = configured else { return };
    if REGISTERED_SHORTCUT.lock().as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&s)) {
        return;
    }
    if let Err(e) = set_shortcut(app, &s) {
        eprintln!("[shortcut] 迷你窗快捷键「{s}」重注册失败: {e}");
    }
}
