//! 在线刮削（M4）：iTunes Search（JP 店面优先——J-Pop 元数据与封面最全，US 兜底）
//! 与 LRCLIB 带时间轴歌词。仅元数据，不引入在线音源（功能设计文档 §7）。
//! 多站点增强：Deezer（封面/专辑信息）与 MusicBrainz + Cover Art Archive（专辑模式），
//! 全部免鉴权；HTTP 走共享 reqwest Client（默认启用环境变量代理，超时 15s）。

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::{header::CONTENT_TYPE, Client};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::{AlbumCandidate, LyricsCandidate, ScrapeSource};

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const SEARCH_LIMIT: u32 = 12;
/// MusicBrainz 礼貌限速：平均 1 req/s
const MB_MIN_INTERVAL: Duration = Duration::from_millis(1100);

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("TinyMusic/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default()
    })
}

/// artworkUrl100 → 1200x1200 高清版（iTunes 图床已知 URL 规则，无副作用）
pub(crate) fn hires_artwork(url: &str) -> String {
    url.replace("100x100bb", "1200x1200bb")
}

/// 多站点合并检索（专辑/单曲候选）：source 指定单一站点，None = 全部站点
/// 按 iTunes → Deezer → MusicBrainz 顺序合并去重；单站点失败不拖垮整体（部分结果可用），
/// 全部失败才报错。MusicBrainz 仅专辑模式（单曲检索不含封面信息，返回空）。
pub async fn search_candidates(
    term: &str,
    entity: &str,
    source: Option<ScrapeSource>,
) -> AppResult<Vec<AlbumCandidate>> {
    let lists = match source {
        Some(ScrapeSource::Itunes) => vec![search_itunes(term, entity, SEARCH_LIMIT).await?],
        Some(ScrapeSource::Deezer) => vec![search_deezer(entity, term).await?],
        Some(ScrapeSource::MusicBrainz) => vec![search_musicbrainz(entity, term).await?],
        None => {
            let (itunes, deezer, mb) = tokio::join!(
                search_itunes(term, entity, SEARCH_LIMIT),
                search_deezer(entity, term),
                search_musicbrainz(entity, term),
            );
            let mut errors: Vec<String> = Vec::new();
            let mut lists: Vec<Vec<AlbumCandidate>> = Vec::new();
            for r in [itunes, deezer, mb] {
                match r {
                    Ok(v) => lists.push(v),
                    Err(e) => errors.push(e.to_string()),
                }
            }
            if lists.iter().all(|l| l.is_empty()) {
                if let Some(e) = errors.first() {
                    return Err(AppError::Message(format!(
                        "全部来源检索失败，最后一个错误: {e}"
                    )));
                }
            }
            lists
        }
    };
    Ok(merge_candidates(&lists))
}

/// 站点结果合并去重：保持传入顺序优先级，同名同艺人的候选只保留首个（跨站点重复）
fn merge_candidates(lists: &[Vec<AlbumCandidate>]) -> Vec<AlbumCandidate> {
    let mut out: Vec<AlbumCandidate> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for list in lists {
        for c in list {
            let key = (c.name.to_lowercase(), c.artist.to_lowercase());
            if seen.insert(key) {
                out.push(c.clone());
            }
        }
    }
    out
}

/// Deezer 检索（免鉴权）：album → /search/album；song → /search/track（取其专辑封面）
async fn search_deezer(entity: &str, term: &str) -> AppResult<Vec<AlbumCandidate>> {
    let path = if entity == "album" { "album" } else { "track" };
    let resp = client()
        .get(format!("https://api.deezer.com/search/{path}"))
        .query(&[("q", term), ("limit", &SEARCH_LIMIT.to_string())])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::Message(format!("Deezer 接口返回 {}", resp.status())));
    }
    let v: Value = resp.json().await?;
    Ok(parse_deezer_results(&v, entity == "album"))
}

/// Deezer 结果宽松解析：album 取曲目数/发行年；track 取其所属专辑
pub(crate) fn parse_deezer_results(v: &Value, album_mode: bool) -> Vec<AlbumCandidate> {
    v.get("data")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let artwork_url = r
                        .pointer(if album_mode { "/cover_xl" } else { "/album/cover_xl" })
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    let name = if album_mode {
                        r.get("title")?.as_str()?.to_string()
                    } else {
                        r.pointer("/album/title")?.as_str()?.to_string()
                    };
                    let artist = r
                        .pointer("/artist/name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if name.is_empty() || artist.is_empty() {
                        return None;
                    }
                    let year = r
                        .get("release_date")
                        .and_then(Value::as_str)
                        .and_then(|d| d.get(0..4))
                        .and_then(|y| y.parse::<i32>().ok());
                    let track_count = if album_mode {
                        r.get("nb_tracks").and_then(Value::as_i64).map(|n| n as i32)
                    } else {
                        None
                    };
                    Some(AlbumCandidate {
                        provider: "deezer".into(),
                        name,
                        artist,
                        year,
                        genre: None,
                        track_count,
                        artwork_url,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// MusicBrainz 礼貌限速：两次请求间隔 ≥ 1.1s
async fn mb_throttle() {
    static LAST: OnceLock<tokio::sync::Mutex<tokio::time::Instant>> = OnceLock::new();
    let last = LAST.get_or_init(|| tokio::sync::Mutex::new(tokio::time::Instant::now()));
    let mut guard = last.lock().await;
    let elapsed = guard.elapsed();
    if elapsed < MB_MIN_INTERVAL {
        tokio::time::sleep(MB_MIN_INTERVAL - elapsed).await;
    }
    *guard = tokio::time::Instant::now();
}

/// MusicBrainz 检索（免鉴权，需 UA 与 1 req/s 限速）+ Cover Art Archive 封面。
/// 仅专辑模式：单曲模式无稳定封面来源，返回空列表。
async fn search_musicbrainz(entity: &str, term: &str) -> AppResult<Vec<AlbumCandidate>> {
    if entity != "album" {
        return Ok(Vec::new());
    }
    mb_throttle().await;
    let resp = client()
        .get("https://musicbrainz.org/ws/2/release/")
        .query(&[("query", term), ("fmt", "json"), ("limit", &SEARCH_LIMIT.to_string())])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::Message(format!("MusicBrainz 接口返回 {}", resp.status())));
    }
    let v: Value = resp.json().await?;
    Ok(parse_musicbrainz_releases(&v))
}

/// MusicBrainz releases 宽松解析：封面经 Cover Art Archive（front 可用时给 front-1200 URL，
/// 应用阶段下载，404 时给出明确报错）
pub(crate) fn parse_musicbrainz_releases(v: &Value) -> Vec<AlbumCandidate> {
    v.get("releases")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let id = r.get("id")?.as_str()?.to_string();
                    let name = r.get("title")?.as_str()?.to_string();
                    let artist = r
                        .pointer("/artist-credit/0/name")
                        .or_else(|| r.pointer("/artist-credit/0/artist/name"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if artist.is_empty() {
                        return None;
                    }
                    let year = r
                        .get("date")
                        .and_then(Value::as_str)
                        .and_then(|d| d.get(0..4))
                        .and_then(|y| y.parse::<i32>().ok());
                    let front = r
                        .pointer("/cover-art-archive/front")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let artwork_url = front.then(|| {
                        format!("https://coverartarchive.org/release/{id}/front-1200")
                    });
                    Some(AlbumCandidate {
                        provider: "musicbrainz".into(),
                        name,
                        artist,
                        year,
                        genre: None,
                        track_count: None,
                        artwork_url,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// iTunes Search：J-Pop 主要来源。JP 店面优先，无结果回退 US 店面。
/// entity = "album"（按专辑刮削）| "song"（按单曲刮削封面）。
pub async fn search_itunes(term: &str, entity: &str, limit: u32) -> AppResult<Vec<AlbumCandidate>> {
    let mut out = search_itunes_store("JP", term, entity, limit).await?;
    if out.is_empty() {
        out = search_itunes_store("US", term, entity, limit).await?;
    }
    Ok(out)
}

async fn search_itunes_store(country: &str, term: &str, entity: &str, limit: u32) -> AppResult<Vec<AlbumCandidate>> {
    let limit = limit.to_string();
    let resp = client()
        .get("https://itunes.apple.com/search")
        .query(&[
            ("term", term),
            ("media", "music"),
            ("entity", entity),
            ("country", country),
            ("limit", limit.as_str()),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::Message(format!("iTunes 接口返回 {}", resp.status())));
    }
    let v: Value = resp.json().await?;
    Ok(parse_itunes_results(&v, country))
}

/// 宽松解析 iTunes results 数组（字段缺失跳过该条）
pub(crate) fn parse_itunes_results(v: &Value, country: &str) -> Vec<AlbumCandidate> {
    let provider = format!("itunes-{}", country.to_lowercase());
    v.get("results")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let name = r.get("collectionName")?.as_str()?.to_string();
                    let artist = r.get("artistName")?.as_str()?.to_string();
                    let year = r
                        .get("releaseDate")
                        .and_then(Value::as_str)
                        .and_then(|d| d.get(0..4))
                        .and_then(|y| y.parse::<i32>().ok());
                    let genre = r.get("primaryGenreName").and_then(Value::as_str).map(str::to_string);
                    let track_count = r.get("trackCount").and_then(Value::as_i64).map(|n| n as i32);
                    let artwork_url =
                        r.get("artworkUrl100").and_then(Value::as_str).map(hires_artwork);
                    Some(AlbumCandidate {
                        provider: provider.clone(),
                        name,
                        artist,
                        year,
                        genre,
                        track_count,
                        artwork_url,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// LRCLIB 歌词搜索（免鉴权）。候选排序：有同步歌词优先，时长与目标一致者优先。
pub async fn search_lyrics(
    track_name: &str,
    artist_name: &str,
    album_name: Option<&str>,
    target_duration: Option<f64>,
) -> AppResult<Vec<LyricsCandidate>> {
    let mut query = vec![("track_name", track_name), ("artist_name", artist_name)];
    if let Some(al) = album_name {
        query.push(("album_name", al));
    }
    let resp = client().get("https://lrclib.net/api/search").query(&query).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Message(format!("LRCLIB 接口返回 {}", resp.status())));
    }
    let v: Value = resp.json().await?;
    let mut out = parse_lrclib(&v);
    sort_candidates(&mut out, target_duration);
    Ok(out)
}

/// 候选排序：有同步歌词优先 → 与目标时长差小者优先
fn sort_candidates(out: &mut [LyricsCandidate], target: Option<f64>) {
    out.sort_by_key(|c| {
        let has_synced = c.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty());
        (std::cmp::Reverse(has_synced), duration_diff(c, target))
    });
}

/// LRCLIB 自由检索（自定义检索词，q 参数模糊匹配）——与标签检索共用解析与排序
pub async fn search_lyrics_q(
    q: &str,
    target_duration: Option<f64>,
) -> AppResult<Vec<LyricsCandidate>> {
    let resp = client()
        .get("https://lrclib.net/api/search")
        .query(&[("q", q)])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::Message(format!("LRCLIB 接口返回 {}", resp.status())));
    }
    let v: Value = resp.json().await?;
    let mut out = parse_lrclib(&v);
    sort_candidates(&mut out, target_duration);
    Ok(out)
}

fn duration_diff(c: &LyricsCandidate, target: Option<f64>) -> i64 {
    match (target, c.duration_sec) {
        (Some(t), Some(d)) => (t - d).abs().round() as i64,
        (Some(_), None) => i64::MAX,
        _ => 0,
    }
}

pub(crate) fn parse_lrclib(v: &Value) -> Vec<LyricsCandidate> {
    v.as_array()
        .map(|arr| arr.iter().filter_map(parse_lrclib_item).collect())
        .unwrap_or_default()
}

fn parse_lrclib_item(r: &Value) -> Option<LyricsCandidate> {
    let id = r.get("id")?.as_i64()? as i32;
    let track_name = r.get("trackName")?.as_str()?.to_string();
    let artist_name = r.get("artistName")?.as_str()?.to_string();
    let nonempty = |s: Option<&str>| s.filter(|s| !s.trim().is_empty()).map(str::to_string);
    Some(LyricsCandidate {
        id,
        track_name,
        artist_name,
        duration_sec: r.get("duration").and_then(Value::as_f64),
        instrumental: r.get("instrumental").and_then(Value::as_bool).unwrap_or(false),
        synced_lyrics: nonempty(r.get("syncedLyrics").and_then(Value::as_str)),
        plain_lyrics: nonempty(r.get("plainLyrics").and_then(Value::as_str)),
    })
}

/// 下载图片（封面）→ (字节, 扩展名)。超限与异常状态报错。
pub async fn download_image(url: &str) -> AppResult<(Vec<u8>, &'static str)> {
    let resp = client().get(url).send().await?.error_for_status()?;
    let ext = match resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
    {
        ct if ct.starts_with("image/png") => "png",
        ct if ct.starts_with("image/jpeg") => "jpg",
        _ => "img",
    };
    let bytes = resp.bytes().await?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(AppError::Message("封面文件过大（>10MB）".into()));
    }
    Ok((bytes.to_vec(), ext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn 高清封面url替换() {
        assert_eq!(
            hires_artwork("https://is1-ssl.mzstatic.com/image/thumb/Music/xx/1.jpg/100x100bb.jpg"),
            "https://is1-ssl.mzstatic.com/image/thumb/Music/xx/1.jpg/1200x1200bb.jpg"
        );
        // 非 100 图床地址原样保留
        assert_eq!(hires_artwork("https://example.com/a.png"), "https://example.com/a.png");
    }

    #[test]
    fn itunes结果解析与年份提取() {
        let v = json!({
            "resultCount": 2,
            "results": [
                {
                    "collectionName": "First Love (Deluxe)",
                    "artistName": "宇多田ヒカル",
                    "releaseDate": "1999-03-10T07:00:07Z",
                    "primaryGenreName": "J-Pop",
                    "trackCount": 15,
                    "artworkUrl100": "https://example.com/100x100bb.jpg"
                },
                { "collectionName": "No Artwork", "artistName": "X" },
                { "artistName": "缺专辑名" }
            ]
        });
        let out = parse_itunes_results(&v, "JP");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].provider, "itunes-jp");
        assert_eq!(out[0].name, "First Love (Deluxe)");
        assert_eq!(out[0].year, Some(1999));
        assert_eq!(out[0].genre.as_deref(), Some("J-Pop"));
        assert_eq!(out[0].track_count, Some(15));
        assert_eq!(out[0].artwork_url.as_deref(), Some("https://example.com/1200x1200bb.jpg"));
        assert_eq!(out[1].year, None);
        assert_eq!(out[1].artwork_url, None);
    }

    #[test]
    fn lrclib解析与排序_同步优先时长贴近() {
        let v = json!([
            {
                "id": 1, "trackName": "Automatic", "artistName": "宇多田ヒカル",
                "duration": 254.0, "instrumental": false,
                "syncedLyrics": null, "plainLyrics": "プレーン歌詞"
            },
            {
                "id": 2, "trackName": "Automatic", "artistName": "宇多田ヒカル",
                "duration": 255.5, "instrumental": false,
                "syncedLyrics": "[00:12.00]automatic", "plainLyrics": null
            },
            {
                "id": 3, "trackName": "Automatic", "artistName": "宇多田ヒカル",
                "duration": 100.0, "instrumental": false,
                "syncedLyrics": "[00:01.00]other", "plainLyrics": null
            },
            { "id": null, "trackName": "x", "artistName": "y" }
        ]);
        let mut out = parse_lrclib(&v);
        let target = Some(254.0);
        sort_candidates(&mut out, target);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id, 2); // 有同步且时长贴近
        assert_eq!(out[1].id, 3); // 有同步但时长差 154s
        assert_eq!(out[2].id, 1); // 仅纯文本
        assert_eq!(out[2].plain_lyrics.as_deref(), Some("プレーン歌詞"));
    }

    #[test]
    fn deezer专辑结果解析() {
        let v = json!({
            "data": [
                {
                    "id": 1, "title": "First Love", "nb_tracks": 15,
                    "release_date": "1999-03-10",
                    "artist": { "name": "宇多田ヒカル" },
                    "cover_xl": "https://e-cdns-images.dzcdn.net/images/cover/xx/1000x1000-000000-80-0-0.jpg"
                },
                { "id": 2, "title": "无封面专辑", "artist": { "name": "X" } },
                { "id": 3, "title": "缺艺人" }
            ]
        });
        let out = parse_deezer_results(&v, true);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].provider, "deezer");
        assert_eq!(out[0].name, "First Love");
        assert_eq!(out[0].artist, "宇多田ヒカル");
        assert_eq!(out[0].year, Some(1999));
        assert_eq!(out[0].track_count, Some(15));
        assert!(out[0].artwork_url.is_some());
        assert_eq!(out[1].artwork_url, None);
    }

    #[test]
    fn deezer单曲结果取所属专辑() {
        let v = json!({
            "data": [
                {
                    "id": 9, "title": "单曲名",
                    "album": { "title": "所属专辑", "cover_xl": "https://example.com/xx.jpg" },
                    "artist": { "name": "Y" }
                }
            ]
        });
        let out = parse_deezer_results(&v, false);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "所属专辑");
        assert_eq!(out[0].artist, "Y");
        assert_eq!(out[0].track_count, None);
        assert_eq!(out[0].artwork_url.as_deref(), Some("https://example.com/xx.jpg"));
    }

    #[test]
    fn musicbrainz结果解析与封面归档() {
        let v = json!({
            "releases": [
                {
                    "id": "mbid-1", "title": "Distance", "date": "2001-03-28",
                    "artist-credit": [ { "name": "宇多田ヒカル" } ],
                    "cover-art-archive": { "front": true }
                },
                {
                    "id": "mbid-2", "title": "无封面", "date": "2002",
                    "artist-credit": [ { "name": "X" } ],
                    "cover-art-archive": { "front": false }
                },
                { "id": "mbid-3", "title": "缺艺人" }
            ]
        });
        let out = parse_musicbrainz_releases(&v);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].provider, "musicbrainz");
        assert_eq!(out[0].name, "Distance");
        assert_eq!(out[0].artist, "宇多田ヒカル");
        assert_eq!(out[0].year, Some(2001));
        assert_eq!(
            out[0].artwork_url.as_deref(),
            Some("https://coverartarchive.org/release/mbid-1/front-1200")
        );
        assert_eq!(out[1].artwork_url, None);
    }

    #[test]
    fn 多站点合并去重保持优先级() {
        let mk = |provider: &str, name: &str, artist: &str| AlbumCandidate {
            provider: provider.into(),
            name: name.into(),
            artist: artist.into(),
            year: None,
            genre: None,
            track_count: None,
            artwork_url: None,
        };
        let lists = vec![
            vec![mk("itunes-jp", "First Love", "宇多田ヒカル"), mk("itunes-us", "Other", "A")],
            vec![
                // 同名同艺人（大小写差异）：应被去重
                mk("deezer", "first love", "宇多田ヒカル"),
                mk("deezer", "Unique", "B"),
            ],
            vec![mk("musicbrainz", "Other", "A")], // 与 iTunes-US 同名：去重
        ];
        let out = merge_candidates(&lists);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].provider, "itunes-jp");
        assert_eq!(out[1].provider, "itunes-us");
        assert_eq!(out[2].provider, "deezer");
    }
}
