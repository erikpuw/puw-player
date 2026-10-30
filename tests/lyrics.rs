use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use puw_player::lyrics::{LyricsRequest, LyricsResolver, LyricsStore, ResolvedLyrics, active_line};

#[derive(Default)]
struct MemoryStore {
    entries: HashMap<i64, (String, String)>,
}

impl LyricsStore for MemoryStore {
    fn cached(&self, track_id: i64) -> Option<(String, String)> {
        self.entries.get(&track_id).cloned()
    }
    fn store(&mut self, track_id: i64, source: &str, content: &str) {
        self.entries
            .insert(track_id, (source.to_owned(), content.to_owned()));
    }
}

type Handler = dyn Fn(&str, &str) -> (u16, Vec<(String, String)>, String) + Send + Sync;

struct MockServer {
    base: String,
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl MockServer {
    fn start(
        delay: Duration,
        handler: impl Fn(&str, &str) -> (u16, Vec<(String, String)>, String) + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        let handler: Arc<Handler> = Arc::new(handler);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                handle_connection(&mut stream, delay, &*handler, &log);
            }
        });
        Self { base, requests }
    }

    fn hits(&self) -> Vec<(String, String)> {
        self.requests.lock().unwrap().clone()
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    delay: Duration,
    handler: &Handler,
    log: &Arc<Mutex<Vec<(String, String)>>>,
) {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 2048];
    while !buffer.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
    }
    let request = String::from_utf8_lossy(&buffer).to_string();
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let target = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let mut user_agent = String::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("user-agent")
        {
            user_agent = value.trim().to_string();
        }
    }
    log.lock()
        .unwrap()
        .push((target.clone(), user_agent.clone()));
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
    let (status, headers, body) = handler(&target, &user_agent);
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str("\r\n");
    response.push_str(&body);
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn request(id: i64, generation: u64, dir: &std::path::Path) -> LyricsRequest<'static> {
    LyricsRequest {
        track_id: id,
        generation,
        path: dir.join("Sau cơn mưa.flac"),
        title: "Sau cơn mưa",
        artist: "An Nhiên",
        album: "Mưa",
        duration_ms: 276_000,
    }
}

const SYNCED: &str = r#"{"id":1,"trackName":"Sau cơn mưa","artistName":"An Nhiên","duration":276.0,"instrumental":false,"plainLyrics":"Mưa rơi\nTrên phố","syncedLyrics":"[00:01.00]Mưa rơi\n[00:04.00]Trên phố"}"#;

#[test]
fn online_fetch_sends_app_agent_and_caches_result() {
    let server = MockServer::start(Duration::ZERO, |target, _| {
        if target.starts_with("/api/get") {
            (200, vec![], SYNCED.into())
        } else {
            (404, vec![], String::new())
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::default();
    let resolver = LyricsResolver::with_endpoint(&server.base, Duration::from_secs(2));
    let resolved = resolver
        .resolve(&request(9, 1, dir.path()), &mut store, true)
        .unwrap();
    assert_eq!(resolved.lines.len(), 2);
    assert_eq!(resolved.lines[1].text, "Trên phố");
    assert_eq!(active_line(&resolved.lines, 4_500, 0), Some(1));
    let hits = server.hits();
    assert!(hits[0].0.starts_with("/api/get?"));
    assert!(hits[0].0.contains("track_name=Sau+c%C6%A1n+m%C6%B0a"));
    assert!(
        hits[0].1.contains("PuwPlayer"),
        "user agent was {:?}",
        hits[0].1
    );
    assert!(!hits[0].0.contains("flac"), "local path must never be sent");
    assert_eq!(store.entries.get(&9).unwrap().0, "lrclib-synced");

    let offline = resolver
        .resolve(&request(9, 1, dir.path()), &mut store, false)
        .unwrap();
    assert_eq!(offline.lines.len(), 2);
    assert_eq!(
        server.hits().len(),
        1,
        "cached lyrics must not hit the network"
    );
}

#[test]
fn search_fallback_matches_title_artist_and_duration() {
    let wrong = r#"{"id":2,"trackName":"Sau cơn mưa","artistName":"An Nhiên","duration":214.0,"instrumental":false,"plainLyrics":"sai","syncedLyrics":"[00:01.00]sai"}"#;
    let right = r#"{"id":3,"trackName":"Sau cơn mưa","artistName":"An Nhiên","duration":275.5,"instrumental":false,"plainLyrics":"đúng","syncedLyrics":"[00:02.00]đúng"}"#;
    let body = format!("[{wrong},{right}]");
    let server = MockServer::start(Duration::ZERO, move |target, _| {
        if target.starts_with("/api/get") {
            (404, vec![], String::new())
        } else {
            (200, vec![], body.clone())
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::default();
    let resolver = LyricsResolver::with_endpoint(&server.base, Duration::from_secs(2));
    let resolved = resolver
        .resolve(&request(1, 1, dir.path()), &mut store, true)
        .unwrap();
    assert_eq!(resolved.lines[0].text, "đúng");
    assert!(server.hits()[1].0.starts_with("/api/search?"));
}

#[test]
fn not_found_and_instrumental_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::default();
    let empty = MockServer::start(Duration::ZERO, |_, _| (404, vec![], String::new()));
    let resolver = LyricsResolver::with_endpoint(&empty.base, Duration::from_secs(2));
    let error = resolver
        .resolve(&request(1, 1, dir.path()), &mut store, true)
        .unwrap_err();
    assert!(error.contains("Không tìm thấy"), "{error}");

    let instrumental = r#"{"id":4,"trackName":"Sau cơn mưa","artistName":"An Nhiên","duration":276.0,"instrumental":true,"plainLyrics":null,"syncedLyrics":null}"#;
    let server = MockServer::start(Duration::ZERO, move |target, _| {
        if target.starts_with("/api/get") {
            (200, vec![], instrumental.into())
        } else {
            (404, vec![], String::new())
        }
    });
    let resolver = LyricsResolver::with_endpoint(&server.base, Duration::from_secs(2));
    let resolved = resolver
        .resolve(&request(2, 1, dir.path()), &mut store, true)
        .unwrap();
    assert!(resolved.instrumental);
    assert!(resolved.lines.is_empty());
}

#[test]
fn transient_server_error_is_retried_once() {
    let dir = tempfile::tempdir().unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = calls.clone();
    let server = MockServer::start(Duration::ZERO, move |_, _| {
        if seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            (503, vec![], String::new())
        } else {
            (200, vec![], SYNCED.into())
        }
    });
    let mut store = MemoryStore::default();
    let resolver = LyricsResolver::with_endpoint(&server.base, Duration::from_secs(2));
    let resolved = resolver
        .resolve(&request(5, 1, dir.path()), &mut store, true)
        .unwrap();
    assert!(!resolved.lines.is_empty());
    assert_eq!(server.hits().len(), 2);

    let down = MockServer::start(Duration::ZERO, |_, _| (503, vec![], String::new()));
    let resolver = LyricsResolver::with_endpoint(&down.base, Duration::from_secs(2));
    let error = resolver
        .resolve(&request(6, 1, dir.path()), &mut store, true)
        .unwrap_err();
    assert!(error.contains("tạm thời"), "{error}");
}

#[test]
fn rate_limit_and_timeout_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let limited = MockServer::start(Duration::ZERO, |_, _| {
        (429, vec![("Retry-After".into(), "7".into())], String::new())
    });
    let mut store = MemoryStore::default();
    let resolver = LyricsResolver::with_endpoint(&limited.base, Duration::from_secs(2));
    let error = resolver
        .resolve(&request(1, 1, dir.path()), &mut store, true)
        .unwrap_err();
    assert!(error.contains('7'), "{error}");

    let slow = MockServer::start(Duration::from_millis(400), |_, _| {
        (200, vec![], SYNCED.into())
    });
    let resolver = LyricsResolver::with_endpoint(&slow.base, Duration::from_millis(120));
    let error = resolver
        .resolve(&request(3, 1, dir.path()), &mut store, true)
        .unwrap_err();
    assert!(
        error.contains("thời gian") || error.contains("timeout"),
        "{error}"
    );
}

#[test]
fn local_lrc_wins_and_offline_skips_network() {
    let dir = tempfile::tempdir().unwrap();
    let track_path = dir.path().join("Sau cơn mưa.flac");
    std::fs::write(&track_path, b"placeholder").unwrap();
    std::fs::write(
        dir.path().join("Sau cơn mưa.lrc"),
        "[00:01.00]Từ máy\n[00:02.50]Câu hai",
    )
    .unwrap();
    let server = MockServer::start(Duration::ZERO, |_, _| (500, vec![], String::new()));
    let resolver = LyricsResolver::with_endpoint(&server.base, Duration::from_secs(2));
    let mut store = MemoryStore::default();
    let resolved = resolver
        .resolve(&request(5, 1, dir.path()), &mut store, true)
        .unwrap();
    assert_eq!(resolved.lines.len(), 2);
    assert_eq!(resolved.source, "local-lrc");
    assert_eq!(
        server.hits().len(),
        0,
        "local file must win before any request"
    );

    let missing = LyricsRequest {
        path: dir.path().join("Khác.flac"),
        ..request(6, 1, dir.path())
    };
    let error = resolver.resolve(&missing, &mut store, false).unwrap_err();
    assert!(
        error.contains("offline") || error.contains("mạng"),
        "{error}"
    );
    assert_eq!(
        server.hits().len(),
        0,
        "offline mode must never hit the network"
    );
}

#[test]
fn stale_responses_are_detected() {
    let fresh = ResolvedLyrics {
        track_id: 2,
        generation: 8,
        source: "lrclib-synced".into(),
        lines: vec![],
        plain: false,
        instrumental: false,
    };
    assert!(fresh.is_current(2, 8));
    assert!(!fresh.is_current(1, 8));
    assert!(!fresh.is_current(2, 7));
}
