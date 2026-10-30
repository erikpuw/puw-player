#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

use crate::i18n::tr;
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
            return Err(tr(
                "Chưa có lời trong bộ nhớ đệm (chế độ offline)",
                "No cached lyrics (offline mode)",
            )
            .into());
        }

        let track = self.lookup(request)?;
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
        Err(tr(
            "LRCLIB không có lời cho bài này",
            "LRCLIB has no lyrics for this track",
        )
        .into())
    }

    /// LRCLIB is often overloaded ("server is busy", 503), and `/api/get` for a track it does not
    /// hold yet triggers a slow upstream lookup that fails first. So: cheap cached lookup, full
    /// lookup, then several searches; a busy step just moves on. Only network errors and rate
    /// limits stop early. Mirrors how PUW Notch finds lyrics.
    fn lookup(&self, request: &LyricsRequest<'_>) -> Result<LrclibTrack, String> {
        let (title, artist) = clean_names(request.title, request.artist);
        let duration = (request.duration_ms / 1000).to_string();
        let exact = [
            ("track_name", title.as_str()),
            ("artist_name", artist.as_str()),
            ("album_name", request.album),
            ("duration", duration.as_str()),
        ];
        let mut busy = None;
        for endpoint in ["get-cached", "get"] {
            match self.get_json::<LrclibTrack>(self.url(endpoint, &exact)?) {
                Ok(Some(track)) => return Ok(track),
                Ok(None) => {}
                Err(Fetch::Busy(status)) => busy = Some(status),
                Err(Fetch::Fatal(error)) => return Err(error),
            }
        }
        let artist_title = format!("{artist} {title}");
        let mut searches = vec![];
        if !artist.is_empty() {
            searches.push(vec![
                ("track_name", title.as_str()),
                ("artist_name", artist.as_str()),
            ]);
            searches.push(vec![("q", artist_title.as_str())]);
        }
        searches.push(vec![("q", title.as_str())]);
        for params in searches {
            match self.get_json::<Vec<LrclibTrack>>(self.url("search", &params)?) {
                Ok(Some(found)) => {
                    if let Some(track) = closest_length(found, request.duration_ms) {
                        return Ok(track);
                    }
                }
                Ok(None) => {}
                Err(Fetch::Busy(status)) => busy = Some(status),
                Err(Fetch::Fatal(error)) => return Err(error),
            }
        }
        Err(match busy {
            Some(status) => tr(
                "Máy chủ LRCLIB đang bận ({status}). Thử bật lại lời sau ít phút.",
                "LRCLIB is busy right now ({status}). Try again in a few minutes.",
            )
            .replace("{status}", &status.to_string()),
            None => tr(
                "Không tìm thấy lời cho bài này",
                "No lyrics found for this track",
            )
            .into(),
        })
    }

    fn url(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<reqwest::Url, String> {
        reqwest::Url::parse_with_params(&format!("{}/api/{endpoint}", self.base), params)
            .map_err(|e| e.to_string())
    }

    /// `Ok(None)` = not found (404). A busy server is retried once before giving up on the step.
    fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: reqwest::Url,
    ) -> Result<Option<T>, Fetch> {
        let mut attempt = 0;
        let response = loop {
            let response = self.client.get(url.clone()).send().map_err(|error| {
                Fetch::Fatal(if error.is_timeout() {
                    tr("Hết thời gian tải lời", "Lyrics download timed out").to_string()
                } else {
                    format!(
                        "{} {error}",
                        tr(
                            "Lỗi mạng khi tải lời:",
                            "Network error while fetching lyrics:"
                        )
                    )
                })
            })?;
            if !response.status().is_server_error() || attempt == 1 {
                break response;
            }
            attempt += 1;
            std::thread::sleep(Duration::from_millis(700));
        };
        match response.status().as_u16() {
            200 => response.json::<T>().map(Some).map_err(|error| {
                Fetch::Fatal(format!(
                    "{} {error}",
                    tr(
                        "Lời trả về không đọc được:",
                        "Could not read the lyrics response:"
                    )
                ))
            }),
            404 => Ok(None),
            429 => {
                let retry = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or(tr("một lúc", "a few"));
                Err(Fetch::Fatal(
                    tr(
                        "LRCLIB giới hạn yêu cầu, thử lại sau {retry} giây",
                        "LRCLIB rate limit reached, try again in {retry} seconds",
                    )
                    .replace("{retry}", retry),
                ))
            }
            status @ 500..=599 => Err(Fetch::Busy(status)),
            status => Err(Fetch::Fatal(format!(
                "{} {status}",
                tr("LRCLIB trả lỗi", "LRCLIB returned error")
            ))),
        }
    }
}

enum Fetch {
    /// 5xx: LRCLIB overloaded; try the next lookup step.
    Busy(u16),
    /// Network down, timeout, rate limit, bad response: stop.
    Fatal(String),
}

/// A version whose length is far from the file is a different cut (extended mix, edit): its
/// timestamps would drift. Closest length wins, within this many seconds.
const LENGTH_TOLERANCE_S: f64 = 20.0;

fn closest_length(found: Vec<LrclibTrack>, duration_ms: u64) -> Option<LrclibTrack> {
    let wanted = duration_ms as f64 / 1000.0;
    let gap = |track: &LrclibTrack| (track.duration - wanted).abs();
    found
        .into_iter()
        .filter(|track| {
            track.instrumental || track.synced_lyrics.is_some() || track.plain_lyrics.is_some()
        })
        .filter(|track| duration_ms == 0 || gap(track) <= LENGTH_TOLERANCE_S)
        .min_by(|a, b| gap(a).total_cmp(&gap(b)))
}

/// Tag noise that breaks lookups: "(Official MV)", "[Lyrics]", "feat. X", "Artist - Topic".
pub fn clean_names(title: &str, artist: &str) -> (String, String) {
    let mut cleaned = String::new();
    let mut depth = 0usize;
    for c in title.chars() {
        match c {
            '(' | '[' | '【' => depth += 1,
            ')' | ']' | '】' => depth = depth.saturating_sub(1),
            _ if depth == 0 => cleaned.push(c),
            _ => {}
        }
    }
    let lower = cleaned.to_lowercase();
    if let Some(cut) = [" feat.", " feat ", " ft.", " ft "]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .min()
    {
        cleaned.truncate(cut);
    }
    let artist = artist
        .trim()
        .trim_end_matches(" - Topic")
        .trim_end_matches(" Official");
    (cleaned.trim().to_string(), artist.trim().to_string())
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
