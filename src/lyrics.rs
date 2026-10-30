#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

pub const USER_AGENT: &str = "PuwPlayer/0.1 (Windows offline music player)";
pub const DEFAULT_ENDPOINT: &str = "https://lrclib.net";

pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let offset = text
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("[offset:")?
                .strip_suffix(']')?
                .parse::<i64>()
                .ok()
        })
        .unwrap_or(0);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim().trim_start_matches('\u{feff}');
        let mut times = Vec::new();
        while let Some(s) = rest.strip_prefix('[') {
            let Some((tag, tail)) = s.split_once(']') else {
                break;
            };
            if let Some((minutes, seconds)) = tag.split_once(':')
                && let (Ok(m), Ok(s)) = (minutes.parse::<u64>(), seconds.parse::<f64>())
                && s.is_finite()
                && (0.0..60.0).contains(&s)
                && m < 10000
            {
                let ms = (m * 60000) as i64 + (s * 1000.0).round() as i64;
                times.push(ms.saturating_add(offset).max(0) as u64);
            }
            rest = tail;
        }
        for time_ms in times {
            lines.push(LyricLine {
                time_ms,
                text: rest.trim().to_owned(),
            });
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

/// Positive adjustment advances the displayed lyrics; negative delays them.
pub fn active_line(lines: &[LyricLine], position_ms: u64, offset_ms: i64) -> Option<usize> {
    let time = (position_ms.min(i64::MAX as u64) as i64).saturating_add(offset_ms);
    if time < 0 {
        return None;
    }
    lines
        .partition_point(|l| l.time_ms <= time as u64)
        .checked_sub(1)
}

pub struct LyricsRequest<'a> {
    pub track_id: i64,
    pub generation: u64,
    pub path: PathBuf,
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub duration_ms: u64,
}

pub trait LyricsStore {
    fn cached(&self, track_id: i64) -> Option<(String, String)>;
    fn store(&mut self, track_id: i64, source: &str, content: &str);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedLyrics {
    pub track_id: i64,
    pub generation: u64,
    pub source: String,
    pub lines: Vec<LyricLine>,
    pub plain: bool,
    pub instrumental: bool,
}

impl ResolvedLyrics {
    /// Guards the UI against a slow response for a track the user already left.
    pub fn is_current(&self, track_id: i64, generation: u64) -> bool {
        self.track_id == track_id && self.generation == generation
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    track_name: String,
    artist_name: String,
    duration: f64,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
}

pub struct LyricsResolver {
    client: reqwest::blocking::Client,
    base: String,
    #[allow(dead_code)]
    timeout: Duration,
}

impl Default for LyricsResolver {
    fn default() -> Self {
        Self::with_endpoint(DEFAULT_ENDPOINT, Duration::from_secs(10))
    }
}

impl LyricsResolver {
    pub fn with_endpoint(base: &str, timeout: Duration) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            client,
            base: base.trim_end_matches('/').to_owned(),
            timeout,
        }
    }

    /// Order: local `.lrc`, cached copy, then LRCLIB when the user enabled online lookup.
    pub fn resolve(
        &self,
        request: &LyricsRequest<'_>,
        store: &mut dyn LyricsStore,
        online: bool,
    ) -> Result<ResolvedLyrics, String> {
        if let Some(local) = read_local(request) {
            return Ok(local);
        }
        if let Some((source, content)) = store.cached(request.track_id) {
            return Ok(from_cache(request, &source, &content));
        }
        if !online {
            return Err("Chưa có lời trong bộ nhớ đệm (chế độ offline)".into());
        }

        let track = match self.fetch_by_metadata(request)? {
            Some(track) => track,
            None => self
                .search(request)?
                .ok_or_else(|| "Không tìm thấy lời cho bài này".to_string())?,
        };
        if track.instrumental {
            store.store(request.track_id, "lrclib-instrumental", "");
            return Ok(ResolvedLyrics {
                track_id: request.track_id,
                generation: request.generation,
                source: "lrclib-instrumental".into(),
                lines: Vec::new(),
                plain: false,
                instrumental: true,
            });
        }
        if let Some(synced) = track.synced_lyrics.filter(|text| !text.trim().is_empty()) {
            store.store(request.track_id, "lrclib-synced", &synced);
            return Ok(ResolvedLyrics {
                track_id: request.track_id,
                generation: request.generation,
                source: "lrclib-synced".into(),
                lines: parse_lrc(&synced),
                plain: false,
                instrumental: false,
            });
        }
        if let Some(plain) = track.plain_lyrics.filter(|text| !text.trim().is_empty()) {
            store.store(request.track_id, "lrclib-plain", &plain);
            return Ok(plain_lyrics(request, "lrclib-plain", &plain));
        }
        Err("LRCLIB không có lời cho bài này".into())
    }

    fn fetch_by_metadata(
        &self,
        request: &LyricsRequest<'_>,
    ) -> Result<Option<LrclibTrack>, String> {
        let duration = (request.duration_ms / 1000).to_string();
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api/get", self.base),
            &[
                ("artist_name", request.artist),
                ("track_name", request.title),
                ("album_name", request.album),
                ("duration", duration.as_str()),
            ],
        )
        .map_err(|e| e.to_string())?;
        self.get_json::<LrclibTrack>(url)
    }

    fn search(&self, request: &LyricsRequest<'_>) -> Result<Option<LrclibTrack>, String> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api/search", self.base),
            &[
                ("track_name", request.title),
                ("artist_name", request.artist),
            ],
        )
        .map_err(|e| e.to_string())?;
        let Some(candidates) = self.get_json::<Vec<LrclibTrack>>(url)? else {
            return Ok(None);
        };
        let wanted = request.duration_ms as f64 / 1000.0;
        Ok(candidates
            .into_iter()
            .filter(|candidate| candidate.track_name.eq_ignore_ascii_case(request.title))
            .filter(|candidate| candidate.artist_name.eq_ignore_ascii_case(request.artist))
            .filter(|candidate| (candidate.duration - wanted).abs() <= 5.0)
            .min_by(|a, b| {
                (a.duration - wanted)
                    .abs()
                    .partial_cmp(&(b.duration - wanted).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }))
    }

    fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: reqwest::Url,
    ) -> Result<Option<T>, String> {
        let response = self.client.get(url).send().map_err(|error| {
            if error.is_timeout() {
                "Hết thời gian tải lời".to_string()
            } else {
                format!("Lỗi mạng khi tải lời: {error}")
            }
        })?;
        match response.status().as_u16() {
            200 => response
                .json::<T>()
                .map(Some)
                .map_err(|error| format!("Lời trả về không đọc được: {error}")),
            404 => Ok(None),
            429 => {
                let retry = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("một lúc");
                Err(format!("LRCLIB giới hạn yêu cầu, thử lại sau {retry} giây"))
            }
            status => Err(format!("LRCLIB trả lỗi {status}")),
        }
    }
}

fn read_local(request: &LyricsRequest<'_>) -> Option<ResolvedLyrics> {
    let path = request.path.with_extension("lrc");
    let bytes = std::fs::read(path).ok()?;
    let content = String::from_utf8_lossy(&bytes);
    if content.trim().is_empty() {
        return None;
    }
    let lines = parse_lrc(&content);
    if lines.is_empty() {
        return Some(plain_lyrics(request, "local-plain", &content));
    }
    Some(ResolvedLyrics {
        track_id: request.track_id,
        generation: request.generation,
        source: "local-lrc".into(),
        lines,
        plain: false,
        instrumental: false,
    })
}

fn from_cache(request: &LyricsRequest<'_>, source: &str, content: &str) -> ResolvedLyrics {
    if source == "lrclib-instrumental" {
        return ResolvedLyrics {
            track_id: request.track_id,
            generation: request.generation,
            source: source.into(),
            lines: Vec::new(),
            plain: false,
            instrumental: true,
        };
    }
    let lines = parse_lrc(content);
    if lines.is_empty() {
        return plain_lyrics(request, source, content);
    }
    ResolvedLyrics {
        track_id: request.track_id,
        generation: request.generation,
        source: source.into(),
        lines,
        plain: false,
        instrumental: false,
    }
}

fn plain_lyrics(request: &LyricsRequest<'_>, source: &str, content: &str) -> ResolvedLyrics {
    let lines = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|text| LyricLine {
            time_ms: 0,
            text: text.to_owned(),
        })
        .collect();
    ResolvedLyrics {
        track_id: request.track_id,
        generation: request.generation,
        source: source.into(),
        lines,
        plain: true,
        instrumental: false,
    }
}
