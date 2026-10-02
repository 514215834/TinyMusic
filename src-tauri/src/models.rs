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

/// 智能歌单规则字段（功能设计文档 M4：五类规则）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SmartField {
    /// 播放次数（play_history 聚合）
    PlayCount,
    /// 最近播放：最近 N 天内播过
    LastPlayed,
    /// 流派（contains / equals）
    Genre,
    /// 年份
    Year,
    /// 添加时间：最近 N 天内入库
    AddedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SmartOp {
    /// 数值/年份 ≥
    Gte,
    /// 数值/年份 ≤
    Lte,
    /// 流派等于
    Eq,
    /// 流派包含
    Contains,
    /// 时间字段：最近 N 天内
    WithinDays,
}

/// 单条规则：field + op + 数值或文本（二选一，按 field 语义取用）
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SmartRule {
    pub field: SmartField,
    pub op: SmartOp,
    /// 数值条件（playCount / year / withinDays 的天数）
    pub num: Option<f64>,
    /// 文本条件（genre）
    pub text: Option<String>,
}

/// 智能歌单（列表实时生成：smart_playlist_tracks 每次按 rules 现算）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SmartPlaylist {
    pub id: i32,
    pub name: String,
    pub rules: Vec<SmartRule>,
    pub track_limit: Option<i32>,
    pub track_count: i32,
}

/// 标签编辑（M4）：全字段整体提交，null = 清除该字段，写回源文件后同步曲库
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TagPatch {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_no: Option<i32>,
}

/// 刮削来源站点（免鉴权主流源）；None = 全部站点合并检索
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ScrapeSource {
    /// iTunes Search（JP 店面优先，US 兜底）：封面/专辑信息
    Itunes,
    /// Deezer API：封面/专辑信息
    Deezer,
    /// MusicBrainz + Cover Art Archive：专辑信息/封面（仅专辑模式，1 req/s 限速）
    MusicBrainz,
}

/// iTunes 刮削候选（专辑/单曲封面与专辑信息，iTunes Search API）
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AlbumCandidate {
    /// 来源店面："itunes-jp" | "itunes-us"（J-Pop 优先 JP 店面，US 兜底）
    pub provider: String,
    pub name: String,
    pub artist: String,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub track_count: Option<i32>,
    /// 高清封面 URL（artworkUrl100 替换为 1200x1200）
    pub artwork_url: Option<String>,
}

/// LRCLIB 歌词候选（带完整文本，应用时整体回传落库）
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricsCandidate {
    pub id: i32,
    pub track_name: String,
    pub artist_name: String,
    pub duration_sec: Option<f64>,
    pub instrumental: bool,
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
}

/// 批量标签编辑（M6）：None = 不修改该字段（区别于单曲编辑的 null=清除）；
/// 曲号重编 = track_no_start 起按传入顺序重编
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchTagPatch {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_no_start: Option<i32>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchTagResult {
    pub updated: Vec<Track>,
    pub failed: u32,
    pub first_error: Option<String>,
}

/// 重复曲目组（M6）：items 按添加时间升序，items[0] 为保留候选
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub title: String,
    pub artist: String,
    pub items: Vec<DuplicateItem>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateItem {
    pub track: Track,
    /// 字节数（f64 承载，specta 禁 i64）
    pub size: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateResolve {
    pub removed: u32,
    pub failed: u32,
    pub first_error: Option<String>,
}

/// 拖拽导入（M7）：新增曲库目录数 / 跳过数（已被曲库目录覆盖或不支持的路径）
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DropImportResult {
    pub folders_added: u32,
    pub folders_skipped: u32,
}
