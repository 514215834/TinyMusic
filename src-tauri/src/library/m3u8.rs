//! M3U8 歌单解析与写出（M3，技术设计文档 §12）。
//! 仅承诺 UTF-8 编码的 .m3u8（规范要求）；foobar2000 等导出的
//! 绝对路径 + #EXTINF 行可无损往返。解析为纯函数，便于单测。

use crate::models::Track;

/// 单条解析结果：路径必有，#EXTINF 元数据可选
#[derive(Debug, Clone, PartialEq)]
pub struct M3u8Entry {
    pub path: String,
    pub title: Option<String>,
    pub duration_sec: Option<f64>,
}

/// Windows 路径归一化：去引号、统一 `\`、小写——大小写/分隔符差异不影响匹配
pub fn normalize_path(path: &str) -> String {
    path.trim().trim_matches('"').replace('/', "\\").to_lowercase()
}

/// 曲目列表 → #EXTM3U 文本（绝对路径；时长缺省写 -1）
pub fn format_m3u8(tracks: &[Track]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for track in tracks {
        let dur = track
            .duration_sec
            .map(|d| d.round() as i64)
            .unwrap_or(-1)
            .max(-1);
        let label = match &track.artist {
            Some(artist) => format!("{artist} - {}", track.title),
            None => track.title.clone(),
        };
        out.push_str(&format!("#EXTINF:{dur},{label}\n"));
        out.push_str(track.path.trim_end());
        out.push('\n');
    }
    out
}

/// #EXTM3U 文本 → 条目列表；非法/注释行跳过，path 行携带此前最近的 #EXTINF 元数据
pub fn parse_m3u8(text: &str) -> Vec<M3u8Entry> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut entries = Vec::new();
    let mut pending: Option<(Option<f64>, Option<String>)> = None;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            // #EXTINF:<时长>,<标题>；时长可缺失（"#EXTINF:,标题"）
            let (dur, title) = match rest.split_once(',') {
                Some((d, t)) => (d.trim().parse::<f64>().ok(), Some(t.trim().to_string())),
                None => (rest.trim().parse::<f64>().ok(), None),
            };
            pending = Some((dur, title));
        } else if line.starts_with('#') {
            // #EXTM3U / #PLAYLIST: 等其他指令：忽略
            continue;
        } else {
            let (dur, title) = pending.take().unwrap_or((None, None));
            // M3U8 惯例：-1 表示未知时长，归一为 None
            let dur = dur.filter(|d| *d >= 0.0);
            entries.push(M3u8Entry {
                path: line.trim().to_string(),
                title,
                duration_sec: dur,
            });
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(path: &str, artist: Option<&str>, title: &str, dur: Option<f64>) -> Track {
        Track {
            id: 0,
            path: path.into(),
            title: title.into(),
            artist: artist.map(Into::into),
            album: None,
            track_no: None,
            duration_sec: dur,
            sample_rate: None,
            bitrate: None,
            year: None,
            genre: None,
            cover_file: None,
        }
    }

    #[test]
    fn 格式化后可完整解析回原曲目() {
        let tracks = vec![
            track(r"C:\mu sic\晴天.flac", Some("周杰伦"), "晴天", Some(269.4)),
            track("C:\\m\\b.mp3", None, "Only Title", None),
        ];
        let text = format_m3u8(&tracks);
        assert!(text.starts_with("#EXTM3U\n"));
        assert!(text.contains("#EXTINF:269,周杰伦 - 晴天\nC:\\mu sic\\晴天.flac\n"));
        assert!(text.contains("#EXTINF:-1,Only Title\n"));

        let entries = parse_m3u8(&text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, r"C:\mu sic\晴天.flac");
        assert_eq!(entries[0].title.as_deref(), Some("周杰伦 - 晴天"));
        assert_eq!(entries[0].duration_sec, Some(269.0));
        assert_eq!(entries[1].path, "C:\\m\\b.mp3");
        assert_eq!(entries[1].duration_sec, None);
    }

    #[test]
    fn 解析容忍文件头注释与缺元数据行() {
        let text = "\u{feff}#EXTM3U\r\n\
                    #PLAYLIST:我的歌单\r\n\
                    \r\n\
                    #EXTINF:12,x\r\n\
                    C:\\a.mp3\r\n\
                    C:/b.flac\r\n\
                    # 一句不是 EXTINF 的注释\r\n\
                    #EXTINF:,无时长标题\r\n\
                    c.mp3\r\n";
        let entries = parse_m3u8(text);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].path, r"C:\a.mp3");
        assert_eq!(entries[0].duration_sec, Some(12.0));
        assert_eq!(entries[1].path, "C:/b.flac");
        assert_eq!(entries[1].duration_sec, None);
        assert_eq!(entries[2].title.as_deref(), Some("无时长标题"));
        assert_eq!(entries[2].duration_sec, None);
    }

    #[test]
    fn 路径归一化忽略大小写斜杠与引号() {
        assert_eq!(normalize_path("C:/Music/X.FLAC"), normalize_path("c:\\music\\x.flac"));
        assert_eq!(normalize_path("\"C:\\a b\\曲.mp3\""), r"c:\a b\曲.mp3");
        assert_ne!(normalize_path("C:\\a.mp3"), normalize_path("C:\\b.mp3"));
    }
}
