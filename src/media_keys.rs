//! Windows media controls (SMTC): publishes what is playing so the volume flyout and
//! now-playing readers such as puw-notch can see it, and forwards their buttons back to us.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use puw_player::i18n::tr;
use puw_player::library::Track;

/// Position drift beyond this (a seek, a stall) is re-published; normal playback is
/// extrapolated by readers from the last update, so steady progress needs no traffic.
const DRIFT_MS: i64 = 1500;

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

pub struct SystemMedia {
    controls: Option<MediaControls>,
    tx: Sender<MediaControlEvent>,
    rx: Receiver<MediaControlEvent>,
    track: Option<Track>,
    cover_file: Option<PathBuf>,
    published: Option<(Status, u64, Instant)>,
}

impl SystemMedia {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            controls: None,
            tx,
            rx,
            track: None,
            cover_file: None,
            published: None,
        }
    }

    /// Registers with SMTC for the given window. Returns an error message when Windows refuses.
    pub fn attach(&mut self, hwnd: *mut std::ffi::c_void) -> Result<(), String> {
        let config = PlatformConfig {
            display_name: "Puw Player",
            dbus_name: "puw_player",
            hwnd: Some(hwnd),
        };
        let mut controls = MediaControls::new(config).map_err(|e| format!("{e:?}"))?;
        let tx = self.tx.clone();
        controls
            .attach(move |event| {
                let _ = tx.send(event);
            })
            .map_err(|e| format!("{e:?}"))?;
        self.controls = Some(controls);
        if let Some(track) = self.track.clone() {
            self.publish_metadata(&track);
        }
        Ok(())
    }

    /// Button presses and seek requests received since the last call.
    pub fn events(&self) -> Vec<MediaControlEvent> {
        self.rx.try_iter().collect()
    }

    /// New track: text and duration now, artwork when it is decoded (`set_cover`).
    pub fn set_track(&mut self, track: &Track, cover: Option<&slint::Image>) {
        self.track = Some(track.clone());
        self.replace_cover_file(None);
        if let Some(image) = cover {
            self.replace_cover_file(write_cover(track.id, image));
        }
        self.publish_metadata(track);
        self.published = None;
    }

    pub fn set_cover(&mut self, track_id: i64, image: &slint::Image) {
        let Some(track) = self.track.clone().filter(|t| t.id == track_id) else {
            return;
        };
        if self.cover_file.is_some() {
            return;
        }
        self.replace_cover_file(write_cover(track_id, image));
        if self.cover_file.is_some() {
            self.publish_metadata(&track);
        }
    }

    /// Called every UI tick; only talks to Windows when status changes or position jumps.
    pub fn update_playback(&mut self, status: Status, position_ms: u64) {
        let now = Instant::now();
        let unchanged = self.published.is_some_and(|(last, at_ms, at)| {
            let expected = if last == Status::Playing {
                at_ms + now.duration_since(at).as_millis() as u64
            } else {
                at_ms
            };
            last == status && (position_ms as i64 - expected as i64).abs() < DRIFT_MS
        });
        if unchanged {
            return;
        }
        self.published = Some((status, position_ms, now));
        let Some(controls) = self.controls.as_mut() else {
            return;
        };
        let progress = Some(MediaPosition(Duration::from_millis(position_ms)));
        let playback = match status {
            Status::Playing => MediaPlayback::Playing { progress },
            Status::Paused => MediaPlayback::Paused { progress },
            Status::Stopped => MediaPlayback::Stopped,
        };
        let _ = controls.set_playback(playback);
    }

    fn publish_metadata(&mut self, track: &Track) {
        let Some(controls) = self.controls.as_mut() else {
            return;
        };
        let cover_url = self
            .cover_file
            .as_ref()
            .map(|path| format!("file://{}", path.display()));
        let _ = controls.set_metadata(MediaMetadata {
            title: Some(non_empty(
                &track.title,
                tr("Không rõ tên bài", "Unknown title"),
            )),
            artist: Some(&track.artist)
                .filter(|a| !a.is_empty())
                .map(String::as_str),
            album: Some(&track.album)
                .filter(|a| !a.is_empty())
                .map(String::as_str),
            cover_url: cover_url.as_deref(),
            // Always set: souvlaki writes EndTime from this, and 0 would hide the progress bar.
            duration: Some(Duration::from_millis(track.duration_ms)),
        });
    }

    /// Keeps one artwork file per track; Windows may read it after we return, so a file is
    /// never overwritten in place, only deleted once the next one replaces it.
    fn replace_cover_file(&mut self, next: Option<PathBuf>) {
        if let Some(old) = self.cover_file.take()
            && Some(&old) != next.as_ref()
        {
            let _ = std::fs::remove_file(old);
        }
        self.cover_file = next;
    }
}

impl Drop for SystemMedia {
    fn drop(&mut self) {
        self.replace_cover_file(None);
    }
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn write_cover(track_id: i64, image: &slint::Image) -> Option<PathBuf> {
    let pixels = image.to_rgba8()?;
    let buffer =
        image::RgbaImage::from_raw(pixels.width(), pixels.height(), pixels.as_bytes().to_vec())?;
    let dir = std::env::temp_dir().join("puw-player");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("now-playing-{track_id}.png"));
    buffer.save(&path).ok()?;
    Some(path)
}
