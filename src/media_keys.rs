
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use puw_player::i18n::tr;
use puw_player::library::Track;
use puw_player::queue::RepeatMode;

#[cfg(windows)]
use windows::Media::{MediaPlaybackAutoRepeatMode, SystemMediaTransportControls};

const DRIFT_MS: i64 = 1500;

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

pub enum ModeRequest {
    Shuffle(bool),
    Repeat(RepeatMode),
}

#[cfg(windows)]
fn repeat_to_smtc(mode: RepeatMode) -> MediaPlaybackAutoRepeatMode {
    match mode {
        RepeatMode::Off => MediaPlaybackAutoRepeatMode::None,
        RepeatMode::All => MediaPlaybackAutoRepeatMode::List,
        RepeatMode::One => MediaPlaybackAutoRepeatMode::Track,
    }
}

#[cfg(windows)]
fn repeat_from_smtc(mode: MediaPlaybackAutoRepeatMode) -> RepeatMode {
    if mode == MediaPlaybackAutoRepeatMode::List {
        RepeatMode::All
    } else if mode == MediaPlaybackAutoRepeatMode::Track {
        RepeatMode::One
    } else {
        RepeatMode::Off
    }
}

pub struct SystemMedia {
    controls: Option<MediaControls>,
    #[cfg(windows)]
    smtc: Option<SystemMediaTransportControls>,
    modes_tx: Sender<ModeRequest>,
    modes_rx: Receiver<ModeRequest>,
    published_modes: Option<(bool, RepeatMode)>,
    tx: Sender<MediaControlEvent>,
    rx: Receiver<MediaControlEvent>,
    track: Option<Track>,
    cover_file: Option<PathBuf>,
    published: Option<(Status, u64, Instant)>,
}

impl SystemMedia {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (modes_tx, modes_rx) = mpsc::channel();
        Self {
            controls: None,
            #[cfg(windows)]
            smtc: None,
            modes_tx,
            modes_rx,
            published_modes: None,
            tx,
            rx,
            track: None,
            cover_file: None,
            published: None,
        }
    }

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
        #[cfg(windows)]
        self.attach_modes(hwnd)?;
        Ok(())
    }

    #[cfg(windows)]
    fn attach_modes(&mut self, hwnd: *mut std::ffi::c_void) -> Result<(), String> {
        use windows::Foundation::TypedEventHandler;
        use windows::Media::{
            AutoRepeatModeChangeRequestedEventArgs, ShuffleEnabledChangeRequestedEventArgs,
        };
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
        use windows::core::{Error, Ref, factory};

        let fail = |error: Error| error.to_string();
        let interop = factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()
            .map_err(fail)?;
        let smtc: SystemMediaTransportControls =
            unsafe { interop.GetForWindow(HWND(hwnd)) }.map_err(fail)?;

        let tx = self.modes_tx.clone();
        smtc.ShuffleEnabledChangeRequested(&TypedEventHandler::new(
            move |_, args: Ref<'_, ShuffleEnabledChangeRequestedEventArgs>| {
                let _ = tx.send(ModeRequest::Shuffle(args.ok()?.RequestedShuffleEnabled()?));
                Ok(())
            },
        ))
        .map_err(fail)?;
        let tx = self.modes_tx.clone();
        smtc.AutoRepeatModeChangeRequested(&TypedEventHandler::new(
            move |_, args: Ref<'_, AutoRepeatModeChangeRequestedEventArgs>| {
                let mode = repeat_from_smtc(args.ok()?.RequestedAutoRepeatMode()?);
                let _ = tx.send(ModeRequest::Repeat(mode));
                Ok(())
            },
        ))
        .map_err(fail)?;
        self.smtc = Some(smtc);
        self.published_modes = None;
        Ok(())
    }

    pub fn mode_requests(&self) -> Vec<ModeRequest> {
        self.modes_rx.try_iter().collect()
    }

    pub fn set_modes(&mut self, shuffle: bool, repeat: RepeatMode) {
        if self.published_modes == Some((shuffle, repeat)) {
            return;
        }
        #[cfg(windows)]
        if let Some(smtc) = &self.smtc {
            let _ = smtc.SetShuffleEnabled(shuffle);
            let _ = smtc.SetAutoRepeatMode(repeat_to_smtc(repeat));
            self.published_modes = Some((shuffle, repeat));
        }
    }

    pub fn events(&self) -> Vec<MediaControlEvent> {
        self.rx.try_iter().collect()
    }

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
            duration: Some(Duration::from_millis(track.duration_ms)),
        });
    }

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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn repeat_maps_both_ways() {
        for mode in [RepeatMode::Off, RepeatMode::All, RepeatMode::One] {
            assert_eq!(repeat_from_smtc(repeat_to_smtc(mode)), mode);
        }
        assert_eq!(repeat_to_smtc(RepeatMode::Off), MediaPlaybackAutoRepeatMode::None);
        assert_eq!(repeat_to_smtc(RepeatMode::All), MediaPlaybackAutoRepeatMode::List);
        assert_eq!(repeat_to_smtc(RepeatMode::One), MediaPlaybackAutoRepeatMode::Track);
    }
}
