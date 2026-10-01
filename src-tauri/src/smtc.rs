//! SMTC 系统媒体集成（技术设计文档 §2.3 / §5 smtc_update）：
//! 主窗口 playerStore 变更经 smtc_update 转发给系统媒体浮层；
//! 媒体键点击经 smtc:command 事件回流主窗口执行。
//! windows-rs 自研封装（ADR 风险清单：无官方插件），windows crate 与 tauri 同版本。

use std::sync::OnceLock;

use tauri::{AppHandle, Emitter, Manager};
use windows::core::factory;
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference};
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

use crate::models::SmtcState;

static SMTC: OnceLock<SystemMediaTransportControls> = OnceLock::new();

/// 绑定主窗口 HWND 并启用媒体键
pub fn init(window: &tauri::WebviewWindow, app: AppHandle) {
    if let Err(e) = init_inner(window, &app) {
        eprintln!("[smtc] 初始化失败: {e}");
    }
}

fn init_inner(window: &tauri::WebviewWindow, app: &AppHandle) -> Result<(), String> {
    // tauri 与本 crate 共用 windows 0.62，HWND 类型直接统一
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let interop: ISystemMediaTransportControlsInterop = factory::<
        SystemMediaTransportControls,
        ISystemMediaTransportControlsInterop,
    >()
    .map_err(w2s)?;
    let smtc: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd) }.map_err(w2s)?;

    smtc.SetIsPlayEnabled(true).map_err(w2s)?;
    smtc.SetIsPauseEnabled(true).map_err(w2s)?;
    smtc.SetIsNextEnabled(true).map_err(w2s)?;
    smtc.SetIsPreviousEnabled(true).map_err(w2s)?;

    let app_for_handler = app.clone();
    let handler = TypedEventHandler::<
        SystemMediaTransportControls,
        SystemMediaTransportControlsButtonPressedEventArgs,
    >::new(move |_sender, args| {
        if let Some(args) = args.as_ref() {
            let action = match args.Button() {
                Ok(SystemMediaTransportControlsButton::Play) => Some("toggle"),
                Ok(SystemMediaTransportControlsButton::Pause) => Some("toggle"),
                Ok(SystemMediaTransportControlsButton::Next) => Some("next"),
                Ok(SystemMediaTransportControlsButton::Previous) => Some("prev"),
                _ => None,
            };
            if let Some(action) = action {
                let _ = app_for_handler.emit("smtc:command", action);
            }
        }
        Ok(())
    });
    smtc.ButtonPressed(&handler).map_err(w2s)?;

    let _ = SMTC.set(smtc);
    Ok(())
}

fn w2s(e: windows::core::Error) -> String {
    e.to_string()
}

/// 更新媒体浮层（元数据 + 状态 + 时间轴）。封面缩略图走 WinRT 异步，
/// 统一放后台线程避免阻塞命令调用方。
pub fn update(app: AppHandle, state: SmtcState) {
    std::thread::spawn(move || {
        if let Err(e) = update_inner(&app, state) {
            eprintln!("[smtc] 更新失败: {e}");
        }
    });
}

fn update_inner(app: &AppHandle, state: SmtcState) -> Result<(), String> {
    let Some(smtc) = SMTC.get() else { return Ok(()) };

    let updater = smtc.DisplayUpdater().map_err(w2s)?;
    updater.SetType(MediaPlaybackType::Music).map_err(w2s)?;
    let music = updater.MusicProperties().map_err(w2s)?;
    music
        .SetTitle(&windows::core::HSTRING::from(state.title))
        .map_err(w2s)?;
    music
        .SetArtist(&windows::core::HSTRING::from(
            state.artist.as_deref().unwrap_or("TinyMusic"),
        ))
        .map_err(w2s)?;
    if let Some(cover) = state.cover_file.as_deref() {
        if let Some(bytes) = read_cover(app, cover) {
            set_thumbnail(&updater, &bytes)?;
        }
    }
    updater.Update().map_err(w2s)?;

    smtc.SetPlaybackStatus(if state.is_playing {
        MediaPlaybackStatus::Playing
    } else {
        MediaPlaybackStatus::Paused
    })
    .map_err(w2s)?;

    let timeline = SystemMediaTransportControlsTimelineProperties::new().map_err(w2s)?;
    timeline.SetStartTime(TimeSpan { Duration: 0 }).map_err(w2s)?;
    timeline
        .SetPosition(TimeSpan {
            Duration: (state.position_sec.max(0.0) * 10_000_000.0) as i64,
        })
        .map_err(w2s)?;
    timeline
        .SetEndTime(TimeSpan {
            Duration: (state.duration_sec.max(0.0) * 10_000_000.0) as i64,
        })
        .map_err(w2s)?;
    smtc.UpdateTimelineProperties(&timeline).map_err(w2s)?;
    Ok(())
}

fn set_thumbnail(
    updater: &windows::Media::SystemMediaTransportControlsDisplayUpdater,
    bytes: &[u8],
) -> Result<(), String> {
    let stream = InMemoryRandomAccessStream::new().map_err(w2s)?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(w2s)?;
    writer.WriteBytes(bytes).map_err(w2s)?;
    block_on(writer.StoreAsync().map_err(w2s)?).map_err(w2s)?;
    block_on(writer.FlushAsync().map_err(w2s)?).map_err(w2s)?;
    writer.DetachStream().map_err(w2s)?;
    let reference = RandomAccessStreamReference::CreateFromStream(&stream).map_err(w2s)?;
    updater.SetThumbnail(&reference).map_err(w2s)?;
    Ok(())
}

/// windows-future 0.3 只提供 Future（阻塞式 get() 已移除）。
/// SMTC 更新已在专用线程执行，用 noop-waker 忙等收口异步结果（操作均为短时写入）。
fn block_on<F>(fut: F) -> F::Output
where
    F: std::future::IntoFuture,
    F::IntoFuture: std::future::Future,
{
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    static NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(core::ptr::null(), &NOOP_VTABLE),
        |_| {},
        |_| {},
        |_| {},
    );
    let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &NOOP_VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut.into_future());
    loop {
        match std::future::Future::poll(fut.as_mut(), &mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn read_cover(app: &AppHandle, cover_file: &str) -> Option<Vec<u8>> {
    // 只接受纯文件名，防路径穿越（对齐 covers 命令的约束）
    if cover_file.is_empty() || cover_file.contains('/') || cover_file.contains('\\') {
        return None;
    }
    let state = app.state::<crate::AppState>();
    let path = state.covers_dir.join(cover_file);
    std::fs::read(path).ok()
}
