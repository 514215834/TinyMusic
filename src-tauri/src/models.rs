use serde::{Deserialize, Serialize};

/// 对外 DTO 统一用 i32/u32（specta-typescript 禁止 i64/u64 导出），DB 层内部仍用 i64
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Folder {
    pub id: i32,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i32,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<i32>,
    pub duration_sec: Option<f64>,
    pub sample_rate: Option<i32>,
    pub bitrate: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackPage {
    pub items: Vec<Track>,
    pub total: i32,
}

/// 扫描进度事件载荷（scan:progress）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub done: u32,
    pub total: u32,
}

/// 扫描完成事件载荷（scan:done）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanDone {
    pub added: u32,
    pub updated: u32,
    pub removed: u32,
    pub unchanged: u32,
    pub errors: u32,
    pub duration_ms: u32,
}

/// 专辑卡片（封面墙用，含曲目数）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfo {
    pub id: i32,
    pub name: String,
    pub artist: String,
    pub year: Option<i32>,
    pub cover_file: Option<String>,
    pub track_count: i32,
}

/// 艺人条目（艺人视图用，含曲目/专辑数）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfo {
    pub id: i32,
    pub name: String,
    pub track_count: i32,
    pub album_count: i32,
}

/// 歌单（侧边栏与歌单视图用）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i32,
    pub name: String,
    pub track_count: i32,
    pub created_at: String,
}

/// M3U8 导入结果：added 为按曲库匹配入库的曲目数，skipped 为未命中跳过数
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistImport {
    pub playlist_id: i32,
    pub added: u32,
    pub skipped: u32,
}

/// SMTC 转发载荷（主窗口 playerStore → Rust → 系统媒体浮层）
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SmtcState {
    pub title: String,
    pub artist: Option<String>,
    pub is_playing: bool,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub cover_file: Option<String>,
}
