//! 在线刮削（M4）：iTunes Search（JP 店面优先——J-Pop 元数据与封面最全，US 兜底）。
//! 与 LRCLIB 带时间轴歌词。仅元数据，不引入在线音源（功能设计文档 §7）。
//! HTTP 走共享 reqwest Client（默认启用环境变量代理，超时 15s）。

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::{header::CONTENT_TYPE, Client};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::{AlbumCandidate, LyricsCandidate};

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

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
}
