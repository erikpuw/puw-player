//! SQLite-backed music library, playlists, lyric cache and settings.
//!
//! The library only stores references to files; it never moves or deletes audio.

use crate::i18n::tr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::playback::{inspect_audio, read_tags};
use crate::settings::{self, Settings};

pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "oga", "m4a", "mp4", "aac"];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub codec: String,
    pub lossless: bool,
}

impl Track {
    pub fn missing(&self) -> bool {
        !self.path.is_file()
    }

    /// File properties only; output device details are shown separately in the UI.
    pub fn source_label(&self) -> String {
        let mut parts = vec![self.codec.to_uppercase()];
        if self.bits > 0 {
            parts.push(format!("{}-bit", self.bits));
        }
        if self.sample_rate > 0 {
            parts.push(format_sample_rate(self.sample_rate));
        }
        parts.join(" · ")
    }
}

pub fn format_sample_rate(sample_rate: u32) -> String {
    if sample_rate.is_multiple_of(1000) {
        format!("{} kHz", sample_rate / 1000)
    } else {
        format!("{:.1} kHz", sample_rate as f64 / 1000.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    pub imported: usize,
    pub failed: Vec<(PathBuf, String)>,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS tracks(
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL DEFAULT '',
                artist TEXT NOT NULL DEFAULT '',
                album TEXT NOT NULL DEFAULT '',
                duration_ms INTEGER NOT NULL DEFAULT 0,
                sample_rate INTEGER NOT NULL DEFAULT 0,
                channels INTEGER NOT NULL DEFAULT 0,
                bits INTEGER NOT NULL DEFAULT 0,
                codec TEXT NOT NULL DEFAULT '',
                lossless INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS playlists(
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS playlist_tracks(
                playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                PRIMARY KEY(playlist_id, track_id)
             );
             CREATE TABLE IF NOT EXISTS settings(
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS lyrics_cache(
                track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS lyric_offsets(
                track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                offset_ms INTEGER NOT NULL
             );",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    pub fn upsert_track(&mut self, track: &Track) -> Result<i64, String> {
        let path = normalize_path(&track.path);
        self.conn
            .execute(
                "INSERT INTO tracks(path, title, artist, album, duration_ms, sample_rate, channels, bits, codec, lossless)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(path) DO UPDATE SET
                    title = CASE WHEN excluded.title = '' THEN tracks.title ELSE excluded.title END,
                    artist = CASE WHEN excluded.artist = '' THEN tracks.artist ELSE excluded.artist END,
                    album = CASE WHEN excluded.album = '' THEN tracks.album ELSE excluded.album END,
                    duration_ms = CASE WHEN excluded.duration_ms = 0 THEN tracks.duration_ms ELSE excluded.duration_ms END,
                    sample_rate = CASE WHEN excluded.sample_rate = 0 THEN tracks.sample_rate ELSE excluded.sample_rate END,
                    channels = CASE WHEN excluded.channels = 0 THEN tracks.channels ELSE excluded.channels END,
                    bits = CASE WHEN excluded.bits = 0 THEN tracks.bits ELSE excluded.bits END,
                    codec = CASE WHEN excluded.codec = '' THEN tracks.codec ELSE excluded.codec END,
                    lossless = excluded.lossless",
                params![
                    path,
                    track.title,
                    track.artist,
                    track.album,
                    track.duration_ms as i64,
                    track.sample_rate,
                    track.channels,
                    track.bits,
                    track.codec,
                    track.lossless as i64
                ],
            )
            .map_err(|e| e.to_string())?;
        self.conn
            .query_row(
                "SELECT id FROM tracks WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())
    }

    pub fn tracks(&self, playlist: Option<i64>) -> Result<Vec<Track>, String> {
        let mut tracks = Vec::new();
        match playlist {
            Some(playlist_id) => {
                let mut statement = self
                    .conn
                    .prepare(
                        "SELECT t.id, t.path, t.title, t.artist, t.album, t.duration_ms, t.sample_rate,
                                t.channels, t.bits, t.codec, t.lossless
                         FROM playlist_tracks pt JOIN tracks t ON t.id = pt.track_id
                         WHERE pt.playlist_id = ?1
                         ORDER BY pt.position",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = statement
                    .query_map(params![playlist_id], row_to_track)
                    .map_err(|e| e.to_string())?;
                for row in rows {
                    tracks.push(row.map_err(|e| e.to_string())?);
                }
            }
            None => {
                let mut statement = self
                    .conn
                    .prepare(
                        "SELECT id, path, title, artist, album, duration_ms, sample_rate,
                                channels, bits, codec, lossless
                         FROM tracks
                         ORDER BY title COLLATE NOCASE, id",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = statement
                    .query_map([], row_to_track)
                    .map_err(|e| e.to_string())?;
                for row in rows {
                    tracks.push(row.map_err(|e| e.to_string())?);
                }
            }
        }
        Ok(tracks)
    }

    pub fn track(&self, id: i64) -> Result<Option<Track>, String> {
        self.conn
            .query_row(
                "SELECT id, path, title, artist, album, duration_ms, sample_rate,
                        channels, bits, codec, lossless
                 FROM tracks WHERE id = ?1",
                params![id],
                row_to_track,
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn create_playlist(&mut self, name: &str) -> Result<i64, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr(
                "Tên playlist không được để trống",
                "Playlist name cannot be empty",
            )
            .into());
        }
        self.conn
            .execute("INSERT INTO playlists(name) VALUES(?1)", params![name])
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_playlist(&mut self, playlist: i64, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr(
                "Tên playlist không được để trống",
                "Playlist name cannot be empty",
            )
            .into());
        }
        self.conn
            .execute(
                "UPDATE playlists SET name = ?1 WHERE id = ?2",
                params![name, playlist],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_playlist(&mut self, playlist: i64) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", params![playlist])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Removes a track from the library only; the audio file stays. Playlist entries, cached
    /// lyrics and offsets go with it (ON DELETE CASCADE).
    pub fn remove_track(&mut self, id: i64) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Removes every track whose file no longer exists. Returns the removed ids.
    pub fn remove_missing(&mut self) -> Result<Vec<i64>, String> {
        let missing: Vec<i64> = self
            .tracks(None)?
            .into_iter()
            .filter(Track::missing)
            .map(|track| track.id)
            .collect();
        for id in &missing {
            self.remove_track(*id)?;
        }
        Ok(missing)
    }

    pub fn playlists(&self) -> Result<Vec<Playlist>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT p.id, p.name,
                        (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)
                 FROM playlists p ORDER BY p.id",
            )
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(Playlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    count: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut playlists = Vec::new();
        for row in rows {
            playlists.push(row.map_err(|e| e.to_string())?);
        }
        Ok(playlists)
    }

    pub fn add_to_playlist(&mut self, playlist: i64, track: i64) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position)
                 VALUES(?1, ?2,
                    (SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1))",
                params![playlist, track],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove_from_playlist(&mut self, playlist: i64, track: i64) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                params![playlist, track],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn settings(&self) -> Result<Settings, String> {
        settings::load(&self.conn)
    }

    pub fn save_settings(&mut self, settings: &Settings) -> Result<(), String> {
        settings::save(&self.conn, settings)
    }

    pub fn set_lyric_offset(&mut self, track: i64, offset_ms: i64) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO lyric_offsets(track_id, offset_ms) VALUES(?1, ?2)
                 ON CONFLICT(track_id) DO UPDATE SET offset_ms = excluded.offset_ms",
                params![track, offset_ms],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn lyric_offset(&self, track: i64) -> Result<i64, String> {
        self.conn
            .query_row(
                "SELECT offset_ms FROM lyric_offsets WHERE track_id = ?1",
                params![track],
                |row| row.get(0),
            )
            .optional()
            .map(|offset| offset.unwrap_or(0))
            .map_err(|e| e.to_string())
    }

    pub fn cached_lyrics(&self, track: i64) -> Result<Option<(String, String)>, String> {
        self.conn
            .query_row(
                "SELECT source, content FROM lyrics_cache WHERE track_id = ?1",
                params![track],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn store_lyrics(&mut self, track: i64, source: &str, content: &str) -> Result<(), String> {
        let fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0);
        self.conn
            .execute(
                "INSERT INTO lyrics_cache(track_id, source, content, fetched_at) VALUES(?1, ?2, ?3, ?4)
                 ON CONFLICT(track_id) DO UPDATE SET
                    source = excluded.source,
                    content = excluded.content,
                    fetched_at = excluded.fetched_at",
                params![track, source, content, fetched_at],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn import_file(&mut self, path: &Path) -> Result<i64, String> {
        let info = inspect_audio(path)?;
        let tags = read_tags(path);
        let title = if tags.title.trim().is_empty() {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            tags.title
        };
        let track = Track {
            id: 0,
            path: path.to_path_buf(),
            title,
            artist: tags.artist,
            album: tags.album,
            duration_ms: info.duration_ms(),
            sample_rate: info.sample_rate,
            channels: info.channels,
            bits: info.bits,
            codec: info.codec,
            lossless: info.lossless,
        };
        self.upsert_track(&track)
    }

    pub fn import_folder(&mut self, folder: &Path) -> ImportReport {
        let mut files = Vec::new();
        collect_audio_files(folder, &mut files);
        files.sort();
        let mut report = ImportReport::default();
        for file in files {
            match self.import_file(&file) {
                Ok(_) => report.imported += 1,
                Err(message) => report.failed.push((file, message)),
            }
        }
        report
    }
}

impl crate::lyrics::LyricsStore for Library {
    fn cached(&self, track_id: i64) -> Option<(String, String)> {
        self.cached_lyrics(track_id).ok().flatten()
    }
    fn store(&mut self, track_id: i64, source: &str, content: &str) {
        let _ = self.store_lyrics(track_id, source, content);
    }
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        title: row.get(2)?,
        artist: row.get(3)?,
        album: row.get(4)?,
        duration_ms: row.get::<_, i64>(5)? as u64,
        sample_rate: row.get::<_, i64>(6)? as u32,
        channels: row.get::<_, i64>(7)? as u16,
        bits: row.get::<_, i64>(8)? as u16,
        codec: row.get(9)?,
        lossless: row.get::<_, i64>(10)? != 0,
    })
}

fn normalize_path(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn collect_audio_files(folder: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files(&path, files);
        } else if path
            .extension()
            .map(|extension| {
                AUDIO_EXTENSIONS
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(&extension.to_string_lossy()))
            })
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
}
