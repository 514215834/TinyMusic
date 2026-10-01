use std::path::Path;

use lofty::file::AudioFile;
use lofty::prelude::*;
use lofty::picture::MimeType;
use lofty::tag::ItemKey;

use crate::library::covers;

pub struct ParsedTrack {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<i64>,
    pub disc: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub duration_sec: f64,
    pub sample_rate: Option<i64>,
    pub bitrate: Option<i64>,
    pub cover_file: Option<String>,
    pub lyrics: Option<String>,
}

/// 解析单个音频文件（lofty）；内嵌封面落到 covers 缓存目录
pub fn parse(path: &Path, covers_dir: &Path) -> Result<ParsedTrack, lofty::error::FileParseError> {
    let tagged = lofty::read_from_path(path)?;
    let props = tagged.properties();

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let title = tag
        .and_then(|t| t.title())
        .map(|s| s.into_owned())
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "未知曲目".into())
        });

    let cover_file = tag
        .and_then(|t| t.pictures().first())
        .and_then(|pic| {
            let ext = match pic.mime_type() {
                Some(MimeType::Png) => "png",
                Some(MimeType::Jpeg) => "jpg",
                _ => "img",
            };
            covers::save(covers_dir, pic.data(), ext).ok()
        });

    Ok(ParsedTrack {
        title,
        artist: tag.and_then(|t| t.artist()).map(|s| s.into_owned()),
        album: tag.and_then(|t| t.album()).map(|s| s.into_owned()),
        track_no: tag.and_then(|t| t.track()).map(i64::from),
        disc: tag.and_then(|t| t.disk()).map(i64::from),
        year: tag.and_then(|t| t.date()).map(|d| i64::from(d.year)),
        genre: tag.and_then(|t| t.genre()).map(|s| s.into_owned()),
        duration_sec: props.duration().as_secs_f64(),
        sample_rate: props.sample_rate().filter(|v| *v > 0).map(i64::from),
        bitrate: props.audio_bitrate().filter(|v| *v > 0).map(i64::from),
        cover_file,
        lyrics: tag.and_then(|t| t.get_string(ItemKey::Lyrics)).map(str::to_owned),
    })
}
