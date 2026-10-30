#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod media_keys;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Model, ModelRc, VecModel};

use puw_player::artwork;
use puw_player::i18n::{self, tr};
use puw_player::library::{Library, Playlist, Track};
use puw_player::lyrics::{LyricLine, LyricsRequest, LyricsResolver, ResolvedLyrics, active_line};
use puw_player::playback::{PlaybackCommand, PlaybackState, Player, read_tags};
use puw_player::queue::{Queue, RepeatMode};
use puw_player::settings::{Settings, Theme as AppTheme};

slint::include_modules!();

const THUMB: u32 = 96;
const COVER: u32 = 256;

struct CoverJob {
    path: PathBuf,
    max: u32,
}

struct CoverResult {
    path: PathBuf,
    max: u32,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

struct LyricsResult {
    track_id: i64,
    track_generation: u64,
    request: u64,
    result: Result<ResolvedLyrics, String>,
}

struct ImportResult {
    result: Result<(usize, usize), String>,
}

struct AppState {
    db_path: PathBuf,
    library: Library,
    player: Player,
    queue: Option<Queue>,
    generation: u64,
    ended_generation: u64,
    error_generation: u64,
    failures: usize,
    current_id: Option<i64>,
    repeat: RepeatMode,
    shuffle: bool,
    settings: Settings,
    library_tracks: Vec<Track>,
    playlist_tracks: Vec<Track>,
    playlists: Vec<Playlist>,
    lyrics: Vec<LyricLine>,
    lyrics_rows: Vec<LyricRow>,
    lyrics_plain: bool,
    lyrics_request: u64,
    lyric_offset_ms: i64,
    active_lyric: i32,
    covers: HashMap<(PathBuf, u32), Option<slint::Image>>,
    pending_covers: HashSet<(PathBuf, u32)>,
    // Separate workers so the now-playing cover never waits behind library thumbnails.
    thumb_tx: Sender<CoverJob>,
    cover_tx: Sender<CoverJob>,
    cover_rx: Receiver<CoverResult>,
    lyrics_tx: Sender<LyricsResult>,
    lyrics_rx: Receiver<LyricsResult>,
    import_tx: Sender<ImportResult>,
    import_rx: Receiver<ImportResult>,
    tracks_model: Rc<VecModel<TrackRow>>,
    playlist_model: Rc<VecModel<TrackRow>>,
    lyrics_model: Rc<VecModel<LyricRow>>,
    status_until: Option<Instant>,
    theme_poll: Instant,
    styled: bool,
    media: media_keys::SystemMedia,
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let db_path = data_dir().join("library.db");
    let library = match Library::open(&db_path) {
        Ok(library) => library,
        Err(error) => {
            ui.set_status_message(
                tr(
                    "Không mở được thư viện: {error}. Đang dùng bộ nhớ tạm.",
                    "Could not open the library: {error}. Using temporary memory.",
                )
                .replace("{error}", &error)
                .into(),
            );
            ui.set_has_status(true);
            Library::open(Path::new(":memory:")).expect("in-memory library")
        }
    };

    let (cover_result_tx, cover_rx): (Sender<CoverResult>, Receiver<CoverResult>) = mpsc::channel();
    let thumb_tx = spawn_cover_worker(cover_result_tx.clone());
    let cover_tx = spawn_cover_worker(cover_result_tx);
    let (lyrics_tx, lyrics_rx): (Sender<LyricsResult>, Receiver<LyricsResult>) = mpsc::channel();
    let (import_tx, import_rx): (Sender<ImportResult>, Receiver<ImportResult>) = mpsc::channel();

    let state = Rc::new(RefCell::new(AppState {
        library,
        db_path,
        player: Player::start(),
        queue: None,
        generation: 0,
        ended_generation: u64::MAX,
        error_generation: u64::MAX,
        failures: 0,
        current_id: None,
        repeat: RepeatMode::Off,
        shuffle: false,
        settings: Settings::default(),
        library_tracks: Vec::new(),
        playlist_tracks: Vec::new(),
        playlists: Vec::new(),
        lyrics: Vec::new(),
        lyrics_rows: Vec::new(),
        lyrics_plain: false,
        lyrics_request: 0,
        lyric_offset_ms: 0,
        active_lyric: -1,
        covers: HashMap::new(),
        pending_covers: HashSet::new(),
        thumb_tx,
        cover_tx,
        cover_rx,
        lyrics_tx,
        lyrics_rx,
        import_tx,
        import_rx,
        tracks_model: Rc::new(VecModel::default()),
        playlist_model: Rc::new(VecModel::default()),
        lyrics_model: Rc::new(VecModel::default()),
        status_until: None,
        theme_poll: Instant::now(),
        styled: false,
        media: media_keys::SystemMedia::new(),
    }));

    ui.set_tracks(ModelRc::from(state.borrow().tracks_model.clone()));
    ui.set_playlist_tracks(ModelRc::from(state.borrow().playlist_model.clone()));
    ui.set_lyrics(ModelRc::from(state.borrow().lyrics_model.clone()));

    {
        let mut s = state.borrow_mut();
        s.settings = s.library.settings().unwrap_or_default();
        set_language(&ui, s.settings.english);
        ui.set_lyrics_online(s.settings.lyrics_online);
        ui.set_theme_mode(match s.settings.theme {
            AppTheme::System => 0,
            AppTheme::Light => 1,
            AppTheme::Dark => 2,
        });
        apply_theme(&ui, s.settings.theme);
        ui.set_now_device(device_label(&s.player).into());
        refresh_library(&ui, &mut s);
        refresh_playlists(&ui, &mut s);
    }

    wire(&ui, &state);

    let timer_state = state.clone();
    let timer_ui = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(ui) = timer_ui.upgrade() else { return };
            let mut s = timer_state.borrow_mut();
            poll(&ui, &mut s);
        },
    );

    ui.show()?;
    apply_border_style(&ui, false);
    ui.run()
}

fn wire(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let handle = ui.as_weak();

    macro_rules! on {
        ($method:ident, |$ui:ident, $s:ident| $body:expr) => {{
            let weak = handle.clone();
            let state = state.clone();
            ui.$method(move || {
                let Some(ui_owned) = weak.upgrade() else {
                    return;
                };
                let $ui = &ui_owned;
                let $s = &mut *state.borrow_mut();
                $body
            });
        }};
    }

    on!(on_play_pause, |ui, s| toggle_play(ui, s));
    on!(on_stop, |ui, s| stop_playback(ui, s));
    on!(on_next, |ui, s| next_track(ui, s));
    on!(on_previous, |ui, s| previous_track(ui, s));
    on!(on_seek_backward, |ui, s| seek_relative(ui, s, -10_000));
    on!(on_seek_forward, |ui, s| seek_relative(ui, s, 10_000));
    on!(on_toggle_shuffle, |ui, s| {
        s.shuffle = !s.shuffle;
        if let Some(queue) = s.queue.as_mut() {
            queue.set_shuffle(s.shuffle);
        }
        ui.set_shuffle(s.shuffle);
    });
    on!(on_cycle_repeat, |ui, s| {
        s.repeat = match s.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
        if let Some(queue) = s.queue.as_mut() {
            queue.repeat = s.repeat;
        }
        ui.set_repeat_mode(match s.repeat {
            RepeatMode::Off => 0,
            RepeatMode::All => 1,
            RepeatMode::One => 2,
        });
    });
    on!(on_window_minimize, |ui, _s| ui.window().set_minimized(true));
    on!(on_window_maximize, |ui, _s| {
        let maximized = !ui.window().is_maximized();
        ui.window().set_maximized(maximized);
        ui.set_window_maximized(maximized);
        apply_border_style(ui, maximized);
    });
    on!(on_window_close, |ui, _s| {
        let _ = ui.window().hide();
    });
    on!(on_dismiss_status, |ui, s| {
        ui.set_has_status(false);
        s.status_until = None;
    });
    on!(on_toggle_lyrics, |ui, s| {
        let visible = !ui.get_lyrics_visible();
        ui.set_lyrics_visible(visible);
        if visible {
            s.active_lyric = -1;
            request_lyrics(ui, s);
        }
    });
    on!(on_toggle_settings, |ui, s| {
        let visible = !ui.get_settings_visible();
        ui.set_settings_visible(visible);
        if visible {
            ui.set_lyrics_online(s.settings.lyrics_online);
        }
    });

    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_seek_to(move |seconds| {
        let Some(ui) = weak.upgrade() else { return };
        let s = state_rc.borrow();
        let ms = (seconds.max(0.0) * 1000.0) as u64;
        s.player.send(PlaybackCommand::Seek(ms));
        ui.set_position(seconds);
        ui.set_position_label(format_time(seconds as f64 * 1000.0).into());
    });

    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_set_volume(move |volume| {
        let Some(ui) = weak.upgrade() else { return };
        let s = state_rc.borrow();
        let volume = volume.clamp(0.0, 1.0);
        s.player.send(PlaybackCommand::Volume(volume));
        ui.set_volume(volume);
        ui.set_volume_label(format!("{}%", (volume * 100.0).round() as i32).into());
    });

    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_activate_track(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        let from_playlist = ui.get_tab() == 1 && !s.playlist_tracks.is_empty();
        let tracks = if from_playlist {
            s.playlist_tracks.clone()
        } else {
            s.library_tracks.clone()
        };
        if tracks.is_empty() {
            return;
        }
        let index = index.max(0) as usize;
        let index = index.min(tracks.len() - 1);
        start_queue(&ui, &mut s, &tracks, index);
    });

    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_select_playlist(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        select_playlist(&ui, &mut s, index);
    });

    on!(on_play_playlist, |ui, s| {
        let tracks = s.playlist_tracks.clone();
        if tracks.is_empty() {
            set_status(
                ui,
                s,
                tr(
                    "Playlist này chưa có bài hát.",
                    "This playlist has no tracks yet.",
                )
                .to_string(),
            );
            return;
        }
        let start = play_all_start(s, tracks.len());
        start_queue(ui, s, &tracks, start);
    });

    on!(on_add_files, |ui, s| {
        let files = rfd::FileDialog::new()
            .add_filter(
                tr("Âm thanh", "Audio"),
                puw_player::library::AUDIO_EXTENSIONS,
            )
            .pick_files()
            .unwrap_or_default();
        if !files.is_empty() {
            start_import(ui, s, files, None);
        }
    });
    on!(on_add_folder, |ui, s| {
        let folder = rfd::FileDialog::new().pick_folder();
        if let Some(folder) = folder {
            start_import(ui, s, Vec::new(), Some(folder));
        }
    });

    on!(on_dialog_cancel, |ui, _s| ui.set_dialog_mode(0));
    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_dialog_confirm(move |text| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        let mode = ui.get_dialog_mode();
        ui.set_dialog_mode(0);
        match mode {
            1 => match s.library.create_playlist(&text) {
                Ok(id) => {
                    // Created from the "add to playlist" picker: put that song in right away.
                    let picked = ui.get_picker_track();
                    if picked >= 0 {
                        ui.set_picker_track(-1);
                        add_library_track(&ui, &mut s, picked, id);
                    }
                    refresh_playlists(&ui, &mut s);
                    let index = s.playlists.iter().position(|playlist| playlist.id == id);
                    if let Some(index) = index {
                        select_playlist(&ui, &mut s, index as i32);
                    }
                }
                Err(error) => set_status(&ui, &mut s, error),
            },
            2 => {
                if let Some(playlist) = selected_playlist(&ui, &s) {
                    match s.library.rename_playlist(playlist.id, &text) {
                        Ok(()) => refresh_playlists(&ui, &mut s),
                        Err(error) => set_status(&ui, &mut s, error),
                    }
                }
            }
            3 => {
                if let Some(playlist) = selected_playlist(&ui, &s) {
                    match s.library.delete_playlist(playlist.id) {
                        Ok(()) => {
                            ui.set_current_playlist(-1);
                            ui.set_current_playlist_name("".into());
                            ui.set_current_playlist_count("".into());
                            s.playlist_tracks.clear();
                            s.playlist_model.set_vec(Vec::new());
                            refresh_playlists(&ui, &mut s);
                        }
                        Err(error) => set_status(&ui, &mut s, error),
                    }
                }
            }
            _ => {}
        }
    });

    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_set_language(move |english| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        s.settings.english = english;
        let settings = s.settings.clone();
        let _ = s.library.save_settings(&settings);
        set_language(&ui, english);
        // Text built on the Rust side carries the old language until rebuilt.
        refresh_library(&ui, &mut s);
        refresh_playlists(&ui, &mut s);
        refresh_playlist_tracks(&ui, &mut s);
        ui.set_now_device(device_label(&s.player).into());
        if let Some(track) = s
            .current_id
            .and_then(|id| s.library.track(id).ok().flatten())
        {
            ui.set_now_title(display_title(&track).into());
            ui.set_now_artist(display_artist(&track).into());
        }
        if ui.get_lyrics_visible() {
            request_lyrics(&ui, &mut s);
        }
    });

    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_set_theme(move |mode| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        ui.set_theme_mode(mode);
        s.settings.theme = match mode {
            1 => AppTheme::Light,
            2 => AppTheme::Dark,
            _ => AppTheme::System,
        };
        let settings = s.settings.clone();
        let _ = s.library.save_settings(&settings);
        apply_theme(&ui, s.settings.theme);
    });
    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_set_lyrics_online(move |online| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        ui.set_lyrics_online(online);
        s.settings.lyrics_online = online;
        let settings = s.settings.clone();
        let _ = s.library.save_settings(&settings);
        if online && ui.get_lyrics_visible() {
            request_lyrics(&ui, &mut s);
        }
    });
    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_lyric_click(move |seconds| {
        let Some(ui) = weak.upgrade() else { return };
        let s = state_rc.borrow();
        s.player
            .send(PlaybackCommand::Seek((seconds.max(0) as u64) * 1000));
        ui.set_following(true);
    });
    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_lyric_offset_step(move |delta| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        s.lyric_offset_ms = (s.lyric_offset_ms + i64::from(delta) * 500).clamp(-10_000, 10_000);
        if let Some(id) = s.current_id {
            let offset = s.lyric_offset_ms;
            let _ = s.library.set_lyric_offset(id, offset);
        }
        ui.set_lyric_offset(format!("{:+.1}s", s.lyric_offset_ms as f64 / 1000.0).into());
        refresh_active_lyric(&ui, &mut s, true);
    });
    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_add_track_to_playlist(move |track_index, playlist_index| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        let Some(playlist_id) = s
            .playlists
            .get(playlist_index.max(0) as usize)
            .map(|p| p.id)
        else {
            return;
        };
        add_library_track(&ui, &mut s, track_index, playlist_id);
    });
    let weak = handle.clone();
    let state_rc = state.clone();
    ui.on_remove_playlist_track(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let mut s = state_rc.borrow_mut();
        let Some(playlist) = selected_playlist(&ui, &s) else {
            return;
        };
        let index = index.max(0) as usize;
        let Some(track) = s.playlist_tracks.get(index) else {
            return;
        };
        let track_id = track.id;
        match s.library.remove_from_playlist(playlist.id, track_id) {
            Ok(()) => {
                refresh_playlist_tracks(&ui, &mut s);
                refresh_playlists(&ui, &mut s);
            }
            Err(error) => set_status(&ui, &mut s, error),
        }
    });
}

fn poll(ui: &AppWindow, s: &mut AppState) {
    drain_covers(ui, s);
    drain_lyrics(ui, s);
    drain_import(ui, s);

    let snapshot = s.player.snapshot();
    ui.set_playing(snapshot.state == PlaybackState::Playing);
    if s.current_id.is_some() {
        let position = snapshot.position_ms as f32 / 1000.0;
        ui.set_position(position);
        ui.set_position_label(format_time(snapshot.position_ms as f64).into());
        if snapshot.duration_ms > 0 {
            let duration = snapshot.duration_ms as f32 / 1000.0;
            ui.set_duration(duration);
            ui.set_duration_label(format_time(snapshot.duration_ms as f64).into());
        }
    }

    if snapshot.state == PlaybackState::Ended
        && snapshot.generation == s.generation
        && s.ended_generation != s.generation
    {
        s.ended_generation = s.generation;
        advance_after_end(ui, s);
    }

    if snapshot.state == PlaybackState::Error
        && snapshot.generation == s.generation
        && s.error_generation != s.generation
    {
        s.error_generation = s.generation;
        let message = snapshot
            .error
            .clone()
            .unwrap_or_else(|| tr("Không phát được bài này", "Could not play this track").into());
        handle_playback_error(ui, s, message);
    }

    if snapshot.state == PlaybackState::Playing {
        s.failures = 0;
    }

    refresh_active_lyric(ui, s, false);

    if s.settings.theme == AppTheme::System && s.theme_poll.elapsed() > Duration::from_secs(2) {
        s.theme_poll = Instant::now();
        apply_theme(ui, AppTheme::System);
    }

    if let Some(deadline) = s.status_until
        && Instant::now() >= deadline
    {
        ui.set_has_status(false);
        s.status_until = None;
    }

    if !s.styled && ui.window().is_visible() {
        s.styled = true;
        ui.set_lists_ready(true);
        apply_border_style(ui, ui.window().is_maximized());
        if let Some(hwnd) = window_hwnd(ui)
            && let Err(error) = s.media.attach(hwnd)
        {
            set_status(
                ui,
                s,
                format!(
                    "{} {error}",
                    tr(
                        "Không đăng ký được điều khiển media của Windows:",
                        "Could not register with Windows media controls:"
                    )
                ),
            );
        }
    }

    for event in s.media.events() {
        handle_media_event(ui, s, event);
    }
    if s.current_id.is_some() {
        let status = match snapshot.state {
            PlaybackState::Playing => media_keys::Status::Playing,
            PlaybackState::Paused => media_keys::Status::Paused,
            _ => media_keys::Status::Stopped,
        };
        s.media.update_playback(status, snapshot.position_ms);
    }

    let maximized = ui.window().is_maximized();
    if maximized != ui.get_window_maximized() {
        ui.set_window_maximized(maximized);
        apply_border_style(ui, maximized);
    }
}

fn drain_covers(ui: &AppWindow, s: &mut AppState) {
    while let Ok(result) = s.cover_rx.try_recv() {
        let image = if result.width > 0 {
            let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                &result.rgba,
                result.width,
                result.height,
            );
            Some(slint::Image::from_rgba8(buffer))
        } else {
            None
        };
        s.covers
            .insert((result.path.clone(), result.max), image.clone());
        if let Some(image) = image {
            for (index, track) in s.library_tracks.iter().enumerate() {
                if track.path == result.path && result.max == THUMB {
                    let mut row = s.tracks_model.row_data(index);
                    if let Some(row) = row.as_mut() {
                        row.cover = image.clone();
                        s.tracks_model.set_row_data(index, row.clone());
                    }
                }
            }
            for (index, track) in s.playlist_tracks.iter().enumerate() {
                if track.path == result.path && result.max == THUMB {
                    let mut row = s.playlist_model.row_data(index);
                    if let Some(row) = row.as_mut() {
                        row.cover = image.clone();
                        s.playlist_model.set_row_data(index, row.clone());
                    }
                }
            }
            if let Some(id) = s.current_id
                && result.max == COVER
            {
                match s.library.track(id).ok().flatten() {
                    Some(track) if track.path == result.path => {
                        ui.set_cover(image.clone());
                        s.media.set_cover(id, &image);
                    }
                    _ => {}
                }
                // Refresh neighbors as their artwork arrives.
                set_neighbor_covers(ui, s);
            }
        }
    }
}

fn drain_lyrics(ui: &AppWindow, s: &mut AppState) {
    while let Ok(message) = s.lyrics_rx.try_recv() {
        if message.request != s.lyrics_request || Some(message.track_id) != s.current_id {
            continue;
        }
        match message.result {
            Ok(resolved) => {
                s.lyrics = resolved.lines.clone();
                s.lyrics_plain = resolved.plain;
                s.active_lyric = -1;
                s.lyrics_rows = resolved
                    .lines
                    .iter()
                    .map(|line| LyricRow {
                        text: line.text.clone().into(),
                        active: false,
                        timed: !resolved.plain && line.time_ms > 0,
                        seconds: (line.time_ms / 1000) as i32,
                    })
                    .collect();
                if resolved.instrumental {
                    ui.set_lyrics_instrumental(true);
                    ui.set_lyrics_status(tr("Bản nhạc không lời.", "Instrumental track.").into());
                } else {
                    ui.set_lyrics_instrumental(false);
                    ui.set_lyrics_status(
                        if resolved.lines.is_empty() {
                            tr("Chưa có lời cho bài này.", "No lyrics for this track yet.")
                        } else {
                            ""
                        }
                        .into(),
                    );
                }
                ui.set_lyrics_plain(resolved.plain);
                s.lyrics_model.set_vec(s.lyrics_rows.clone());
                refresh_active_lyric(ui, s, true);
                let _ = message.track_generation;
            }
            Err(error) => {
                s.lyrics.clear();
                s.lyrics_rows.clear();
                s.lyrics_model.set_vec(Vec::new());
                ui.set_lyrics_status(error.into());
                ui.set_lyrics_instrumental(false);
            }
        }
    }
}

fn drain_import(ui: &AppWindow, s: &mut AppState) {
    while let Ok(message) = s.import_rx.try_recv() {
        ui.set_importing(false);
        match message.result {
            Ok((imported, failed)) => {
                refresh_library(ui, s);
                refresh_playlists(ui, s);
                let mut text = tr("Đã thêm {n} bài.", "Added {n} tracks.")
                    .replace("{n}", &imported.to_string());
                if failed > 0 {
                    text.push(' ');
                    text.push_str(
                        &tr("{n} tệp không đọc được.", "{n} files could not be read.")
                            .replace("{n}", &failed.to_string()),
                    );
                }
                set_status(ui, s, text);
            }
            Err(error) => set_status(
                ui,
                s,
                format!("{} {error}", tr("Nhập nhạc thất bại:", "Import failed:")),
            ),
        }
    }
}

fn refresh_library(ui: &AppWindow, s: &mut AppState) {
    s.library_tracks = s.library.tracks(None).unwrap_or_default();
    let tracks = s.library_tracks.clone();
    let rows: Vec<TrackRow> = tracks.iter().map(|track| make_row(s, track)).collect();
    s.tracks_model.set_vec(rows);
    highlight_current(ui, s);
}

fn refresh_playlists(ui: &AppWindow, s: &mut AppState) {
    s.playlists = s.library.playlists().unwrap_or_default();
    let playlists = s.playlists.clone();
    let rows: Vec<PlaylistRow> = playlists
        .iter()
        .map(|playlist| {
            let cover_path = s
                .library
                .tracks(Some(playlist.id))
                .ok()
                .and_then(|tracks| tracks.first().map(|track| track.path.clone()));
            PlaylistRow {
                name: playlist.name.clone().into(),
                count: track_count(playlist.count).into(),
                cover: cover_path
                    .as_deref()
                    .and_then(|path| load_cover_now(path, THUMB))
                    .unwrap_or_default(),
            }
        })
        .collect();
    ui.set_playlists(ModelRc::from(Rc::new(VecModel::from(rows))));
    let index = ui.get_current_playlist();
    if index >= 0 && (index as usize) < s.playlists.len() {
        let playlist = &s.playlists[index as usize];
        ui.set_current_playlist_name(playlist.name.clone().into());
        ui.set_current_playlist_count(track_count(playlist.count).into());
    }
}

fn refresh_playlist_tracks(ui: &AppWindow, s: &mut AppState) {
    let Some(playlist) = selected_playlist(ui, s) else {
        s.playlist_tracks.clear();
        s.playlist_model.set_vec(Vec::new());
        return;
    };
    s.playlist_tracks = s.library.tracks(Some(playlist.id)).unwrap_or_default();
    let tracks = s.playlist_tracks.clone();
    let rows: Vec<TrackRow> = tracks.iter().map(|track| make_row(s, track)).collect();
    s.playlist_model.set_vec(rows);
    ui.set_current_playlist_name(playlist.name.clone().into());
    ui.set_current_playlist_count(track_count(playlist.count).into());
}

fn make_row(s: &mut AppState, track: &Track) -> TrackRow {
    TrackRow {
        title: display_title(track).into(),
        artist: display_artist(track).into(),
        duration: format_time(track.duration_ms as f64).into(),
        cover: ensure_cover(s, &track.path, THUMB).unwrap_or_default(),
        missing: track.missing(),
    }
}

fn ensure_cover(s: &mut AppState, path: &Path, max: u32) -> Option<slint::Image> {
    let key = (path.to_path_buf(), max);
    if let Some(cached) = s.covers.get(&key) {
        return cached.clone();
    }
    if s.pending_covers.insert(key) {
        let tx = if max == THUMB {
            &s.thumb_tx
        } else {
            &s.cover_tx
        };
        let _ = tx.send(CoverJob {
            path: path.to_path_buf(),
            max,
        });
    }
    None
}

fn spawn_cover_worker(results: Sender<CoverResult>) -> Sender<CoverJob> {
    let (tx, jobs) = mpsc::channel::<CoverJob>();
    std::thread::spawn(move || {
        while let Ok(job) = jobs.recv() {
            let (width, height, rgba) =
                cover_bytes(&job.path, job.max).unwrap_or((0, 0, Vec::new()));
            let _ = results.send(CoverResult {
                path: job.path,
                max: job.max,
                width,
                height,
                rgba,
            });
        }
    });
    tx
}

fn highlight_current(ui: &AppWindow, s: &mut AppState) {
    let current = s
        .current_id
        .and_then(|id| s.library_tracks.iter().position(|track| track.id == id))
        .map(|index| index as i32)
        .unwrap_or(-1);
    ui.set_current_track(current);
}

/// "Play all" starts at the top, or at a random track when shuffle is on.
fn play_all_start(s: &AppState, len: usize) -> usize {
    if s.shuffle && len > 0 {
        rand::random_range(0..len)
    } else {
        0
    }
}

fn start_queue(ui: &AppWindow, s: &mut AppState, tracks: &[Track], index: usize) {
    let ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
    let mut queue = Queue::new(ids, index);
    queue.repeat = s.repeat;
    if s.shuffle {
        queue.set_shuffle(true);
    }
    s.queue = Some(queue);
    s.failures = 0;
    load_current(ui, s);
}

fn load_current(ui: &AppWindow, s: &mut AppState) {
    let Some(id) = s.queue.as_ref().and_then(|queue| queue.current()) else {
        return;
    };
    let Ok(Some(track)) = s.library.track(id) else {
        return;
    };
    // Slide the cover flow only when the new track sits next to the old one in the queue.
    let flow_dir = match (s.current_id, s.queue.as_ref()) {
        (Some(old), Some(queue)) if queue.neighbor(-1) == Some(old) => 1.0,
        (Some(old), Some(queue)) if queue.neighbor(1) == Some(old) => -1.0,
        _ => 0.0,
    };
    s.current_id = Some(id);
    s.generation = s.generation.wrapping_add(1);
    s.ended_generation = u64::MAX;
    s.error_generation = u64::MAX;
    s.player.load(id, s.generation, track.path.clone());

    ui.set_has_track(true);
    ui.set_now_title(display_title(&track).into());
    ui.set_now_artist(display_artist(&track).into());
    show_source_quality(ui, &track);
    ui.set_duration(track.duration_ms as f32 / 1000.0);
    ui.set_duration_label(format_time(track.duration_ms as f64).into());
    ui.set_position(0.0);
    ui.set_position_label("00:00".into());
    let cover = ensure_cover(s, &track.path, COVER);
    s.media.set_track(&track, cover.as_ref());
    ui.set_cover(cover.unwrap_or_default());
    set_neighbor_covers(ui, s);
    ui.invoke_start_flow(flow_dir);

    s.lyric_offset_ms = s.library.lyric_offset(id).unwrap_or(0);
    ui.set_lyric_offset(format!("{:+.1}s", s.lyric_offset_ms as f64 / 1000.0).into());
    s.lyrics.clear();
    s.lyrics_rows.clear();
    s.active_lyric = -1;
    s.lyrics_model.set_vec(Vec::new());
    ui.set_lyrics_instrumental(false);
    ui.set_lyrics_status(
        if ui.get_lyrics_visible() {
            tr("Đang tải lời…", "Loading lyrics…")
        } else {
            ""
        }
        .into(),
    );
    if ui.get_lyrics_visible() {
        request_lyrics(ui, s);
    }

    highlight_current(ui, s);
    let playlist_index = s
        .playlist_tracks
        .iter()
        .position(|candidate| candidate.id == id)
        .map(|index| index as i32)
        .unwrap_or(-1);
    ui.set_current_playlist_track(playlist_index);
    let position = s
        .queue
        .as_ref()
        .map(|queue| queue.position() as i32)
        .unwrap_or(-1);
    ui.set_selected_track(position);
}

fn neighbor_cover(s: &mut AppState, delta: isize) -> Option<slint::Image> {
    let id = s.queue.as_ref().and_then(|queue| queue.neighbor(delta))?;
    let track = s.library.track(id).ok().flatten()?;
    ensure_cover(s, &track.path, COVER)
}

fn set_neighbor_covers(ui: &AppWindow, s: &mut AppState) {
    ui.set_cover_prev2(neighbor_cover(s, -2).unwrap_or_default());
    ui.set_cover_prev(neighbor_cover(s, -1).unwrap_or_default());
    ui.set_cover_next(neighbor_cover(s, 1).unwrap_or_default());
    ui.set_cover_next2(neighbor_cover(s, 2).unwrap_or_default());
}

fn toggle_play(ui: &AppWindow, s: &mut AppState) {
    let state = s.player.snapshot().state;
    match state {
        PlaybackState::Playing => {
            s.player.send(PlaybackCommand::Pause);
            ui.set_playing(false);
        }
        // Nothing chosen yet (fresh start is Idle with no queue): play the whole library.
        _ if s.current_id.is_none() => {
            let tracks = s.library_tracks.clone();
            if tracks.is_empty() {
                set_status(
                    ui,
                    s,
                    tr("Thư viện chưa có bài hát.", "Your library is empty.").to_string(),
                );
                return;
            }
            let start = play_all_start(s, tracks.len());
            start_queue(ui, s, &tracks, start);
        }
        PlaybackState::Paused | PlaybackState::Stopped | PlaybackState::Ended => {
            s.player.send(PlaybackCommand::Play);
            ui.set_playing(true);
        }
        PlaybackState::Error | PlaybackState::Idle => load_current(ui, s),
    }
}

fn next_track(ui: &AppWindow, s: &mut AppState) {
    let Some(queue) = s.queue.as_mut() else {
        return;
    };
    if queue.advance(true).is_some() {
        load_current(ui, s);
    }
}

fn previous_track(ui: &AppWindow, s: &mut AppState) {
    let Some(queue) = s.queue.as_mut() else {
        return;
    };
    if queue.previous().is_some() {
        load_current(ui, s);
    }
}

/// Source line + quality badge from a fresh header read, so tracks imported before the
/// codec details were stored (e.g. ALAC inside .m4a) still show the right figures.
fn show_source_quality(ui: &AppWindow, track: &Track) {
    use puw_player::playback::{Quality, inspect_audio};
    let Ok(info) = inspect_audio(&track.path) else {
        ui.set_now_source(track.source_label().into());
        ui.set_now_quality("".into());
        ui.set_now_quality_tier(0);
        return;
    };
    let fresh = Track {
        codec: info.codec.clone(),
        bits: info.bits,
        sample_rate: info.sample_rate,
        lossless: info.lossless,
        ..track.clone()
    };
    let mut source = fresh.source_label();
    let (label, tier) = match info.quality() {
        Quality::HiRes => ("Hi-Res Lossless", 3),
        Quality::Lossless => ("Lossless", 2),
        Quality::Lossy { kbps } => {
            if kbps > 0 {
                source.push_str(&format!(" · {kbps} kbps"));
            }
            ("", 0)
        }
    };
    ui.set_now_source(source.into());
    ui.set_now_quality(label.into());
    ui.set_now_quality_tier(tier);
}

fn stop_playback(ui: &AppWindow, s: &mut AppState) {
    s.player.send(PlaybackCommand::Stop);
    ui.set_playing(false);
}

/// Buttons from the Windows media flyout, keyboard media keys and readers such as puw-notch.
fn handle_media_event(ui: &AppWindow, s: &mut AppState, event: souvlaki::MediaControlEvent) {
    use souvlaki::{MediaControlEvent as E, SeekDirection};
    let playing = s.player.snapshot().state == PlaybackState::Playing;
    match event {
        E::Toggle => toggle_play(ui, s),
        E::Play if !playing => toggle_play(ui, s),
        E::Pause if playing => toggle_play(ui, s),
        E::Next => next_track(ui, s),
        E::Previous => previous_track(ui, s),
        E::Stop => stop_playback(ui, s),
        E::Seek(SeekDirection::Forward) => seek_relative(ui, s, 10_000),
        E::Seek(SeekDirection::Backward) => seek_relative(ui, s, -10_000),
        E::SeekBy(direction, by) => {
            let ms = by.as_millis() as i64;
            seek_relative(
                ui,
                s,
                if direction == SeekDirection::Forward {
                    ms
                } else {
                    -ms
                },
            );
        }
        E::SetPosition(souvlaki::MediaPosition(at)) => {
            s.player.send(PlaybackCommand::Seek(at.as_millis() as u64));
        }
        _ => {}
    }
}

fn seek_relative(ui: &AppWindow, s: &mut AppState, delta_ms: i64) {
    let snapshot = s.player.snapshot();
    let target = (snapshot.position_ms as i64 + delta_ms).max(0) as u64;
    s.player.send(PlaybackCommand::Seek(target));
    let _ = ui;
}

fn advance_after_end(ui: &AppWindow, s: &mut AppState) {
    let Some(queue) = s.queue.as_mut() else {
        ui.set_playing(false);
        return;
    };
    if queue.advance(false).is_some() {
        load_current(ui, s);
    } else {
        ui.set_playing(false);
        let _ = ui;
    }
}

fn handle_playback_error(ui: &AppWindow, s: &mut AppState, message: String) {
    s.failures += 1;
    let queue_len = s.queue.as_ref().map(|queue| queue.len()).unwrap_or(0);
    if queue_len > 1
        && s.failures < queue_len
        && let Some(queue) = s.queue.as_mut()
        && queue.advance(true).is_some()
    {
        load_current(ui, s);
        return;
    }
    s.player.send(PlaybackCommand::Stop);
    ui.set_playing(false);
    set_status(ui, s, message);
}

fn request_lyrics(ui: &AppWindow, s: &mut AppState) {
    let Some(id) = s.current_id else { return };
    let Ok(Some(track)) = s.library.track(id) else {
        return;
    };
    s.lyrics_request = s.lyrics_request.wrapping_add(1);
    let request = s.lyrics_request;
    let generation = s.generation;
    let tx = s.lyrics_tx.clone();
    let db_path = s.db_path.clone();
    let online = s.settings.lyrics_online;
    ui.set_lyrics_status(tr("Đang tải lời…", "Loading lyrics…").into());
    std::thread::spawn(move || {
        let mut library = match Library::open(&db_path) {
            Ok(library) => library,
            Err(error) => {
                let _ = tx.send(LyricsResult {
                    track_id: id,
                    track_generation: generation,
                    request,
                    result: Err(error),
                });
                return;
            }
        };
        let resolver = LyricsResolver::default();
        let lyrics_request = LyricsRequest {
            track_id: id,
            generation,
            path: track.path.clone(),
            title: display_title(&track),
            artist: display_artist(&track),
            album: &track.album,
            duration_ms: track.duration_ms,
        };
        let result = resolver.resolve(&lyrics_request, &mut library, online);
        let _ = tx.send(LyricsResult {
            track_id: id,
            track_generation: generation,
            request,
            result,
        });
    });
}

fn refresh_active_lyric(ui: &AppWindow, s: &mut AppState, force: bool) {
    if s.lyrics.is_empty() || s.lyrics_plain {
        return;
    }
    let position = s.player.snapshot().position_ms;
    let active = active_line(&s.lyrics, position, s.lyric_offset_ms)
        .map(|index| index as i32)
        .unwrap_or(-1);
    if active == s.active_lyric && !force {
        return;
    }
    s.active_lyric = active;
    for (index, row) in s.lyrics_rows.iter_mut().enumerate() {
        row.active = index as i32 == active;
    }
    s.lyrics_model.set_vec(s.lyrics_rows.clone());
    ui.set_active_lyric(active);
}

fn select_playlist(ui: &AppWindow, s: &mut AppState, index: i32) {
    if index < 0 || index as usize >= s.playlists.len() {
        ui.set_current_playlist(-1);
        s.playlist_tracks.clear();
        s.playlist_model.set_vec(Vec::new());
        return;
    }
    ui.set_current_playlist(index);
    refresh_playlist_tracks(ui, s);
}

fn add_library_track(ui: &AppWindow, s: &mut AppState, track_index: i32, playlist_id: i64) {
    let Some(track) = s.library_tracks.get(track_index.max(0) as usize).cloned() else {
        return;
    };
    match s.library.add_to_playlist(playlist_id, track.id) {
        Ok(()) => {
            refresh_playlists(ui, s);
            if selected_playlist(ui, s).is_some_and(|p| p.id == playlist_id) {
                refresh_playlist_tracks(ui, s);
            }
            let name = s
                .playlists
                .iter()
                .find(|p| p.id == playlist_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            set_status(
                ui,
                s,
                tr(
                    "Đã thêm “{title}” vào “{name}”.",
                    "Added “{title}” to “{name}”.",
                )
                .replace("{title}", display_title(&track))
                .replace("{name}", &name),
            );
        }
        Err(error) => set_status(ui, s, error),
    }
}

fn selected_playlist(ui: &AppWindow, s: &AppState) -> Option<Playlist> {
    let index = ui.get_current_playlist();
    if index < 0 {
        return None;
    }
    s.playlists.get(index as usize).cloned()
}

fn start_import(ui: &AppWindow, s: &mut AppState, files: Vec<PathBuf>, folder: Option<PathBuf>) {
    ui.set_importing(true);
    let tx = s.import_tx.clone();
    let db_path = s.db_path.clone();
    std::thread::spawn(move || {
        let result = match Library::open(&db_path) {
            Ok(mut library) => {
                let mut imported = 0;
                let mut failed = 0;
                if let Some(folder) = folder {
                    let report = library.import_folder(&folder);
                    imported += report.imported;
                    failed += report.failed.len();
                }
                for file in files {
                    match library.import_file(&file) {
                        Ok(_) => imported += 1,
                        Err(_) => failed += 1,
                    }
                }
                Ok((imported, failed))
            }
            Err(error) => Err(error),
        };
        let _ = tx.send(ImportResult { result });
    });
}

fn set_status(ui: &AppWindow, s: &mut AppState, message: String) {
    ui.set_status_message(message.into());
    ui.set_has_status(true);
    s.status_until = Some(Instant::now() + Duration::from_secs(7));
}

fn load_cover_now(path: &Path, max: u32) -> Option<slint::Image> {
    let (width, height, rgba) = cover_bytes(path, max)?;
    let buffer =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&rgba, width, height);
    Some(slint::Image::from_rgba8(buffer))
}

fn cover_bytes(path: &Path, max: u32) -> Option<(u32, u32, Vec<u8>)> {
    let artwork = read_tags(path).artwork?;
    let image = artwork::load_cover(&artwork)?;
    let thumb = image.thumbnail(max, max).to_rgba8();
    Some((thumb.width(), thumb.height(), thumb.into_raw()))
}

fn apply_theme(ui: &AppWindow, theme: AppTheme) {
    let dark = match theme {
        AppTheme::Dark => true,
        AppTheme::Light => false,
        AppTheme::System => system_dark(),
    };
    let palette = if dark {
        Palette {
            bg: (0x17, 0x19, 0x1a),
            surface: (0x1e, 0x22, 0x23),
            text: (0xe8, 0xeb, 0xe8),
            muted: (0x97, 0xa0, 0x9c),
            accent: (0xc8, 0xe3, 0x8c),
            accent_soft: (0x2a, 0xc8, 0xe3, 0x8c),
            hover: (0x14, 0xff, 0xff, 0xff),
            line: (0x1f, 0xff, 0xff, 0xff),
            on_accent: (0x17, 0x19, 0x1a),
        }
    } else {
        Palette {
            bg: (0xf4, 0xf5, 0xf2),
            surface: (0xff, 0xff, 0xff),
            text: (0x1c, 0x21, 0x1d),
            muted: (0x65, 0x70, 0x69),
            accent: (0x5f, 0x7d, 0x1f),
            accent_soft: (0x2a, 0x5f, 0x7d, 0x1f),
            hover: (0x0d, 0x00, 0x00, 0x00),
            line: (0x16, 0x00, 0x00, 0x00),
            on_accent: (0xf7, 0xf8, 0xf4),
        }
    };
    let global = ui.global::<Theme>();
    global.set_bg(color_rgb(palette.bg));
    global.set_surface(color_rgb(palette.surface));
    global.set_text(color_rgb(palette.text));
    global.set_muted(color_rgb(palette.muted));
    global.set_accent(color_rgb(palette.accent));
    global.set_accent_soft(color_argb(palette.accent_soft));
    global.set_hover(color_argb(palette.hover));
    global.set_line(color_argb(palette.line));
    global.set_on_accent(color_rgb(palette.on_accent));
    global.set_dark(dark);
    let motion = if animations_enabled() {
        (140, 240)
    } else {
        (0, 0)
    };
    let motion_global = ui.global::<Motion>();
    motion_global.set_fast(motion.0 as i64);
    motion_global.set_slow(motion.1 as i64);
}

struct Palette {
    bg: (u8, u8, u8),
    surface: (u8, u8, u8),
    text: (u8, u8, u8),
    muted: (u8, u8, u8),
    accent: (u8, u8, u8),
    accent_soft: (u8, u8, u8, u8),
    hover: (u8, u8, u8, u8),
    line: (u8, u8, u8, u8),
    on_accent: (u8, u8, u8),
}

fn color_argb((a, r, g, b): (u8, u8, u8, u8)) -> slint::Color {
    slint::Color::from_argb_u8(a, r, g, b)
}
fn color_rgb((r, g, b): (u8, u8, u8)) -> slint::Color {
    slint::Color::from_rgb_u8(r, g, b)
}

fn device_label(player: &Player) -> String {
    let Some((rate, channels)) = player.device() else {
        return String::new();
    };
    tr("Ra · {khz} kHz · {ch} kênh", "Out · {khz} kHz · {ch} ch")
        .replace("{khz}", &(rate / 1000).to_string())
        .replace("{ch}", &channels.to_string())
}

/// Switches both the Rust `tr` texts and every Slint `I18n.t` binding.
fn set_language(ui: &AppWindow, english: bool) {
    i18n::set_english(english);
    ui.global::<I18n>().set_en(english);
}

fn track_count(count: impl std::fmt::Display) -> String {
    let count = count.to_string();
    if count == "1" {
        tr("1 bài", "1 track").to_string()
    } else {
        tr("{n} bài", "{n} tracks").replace("{n}", &count)
    }
}

fn display_title(track: &Track) -> &str {
    if track.title.trim().is_empty() {
        tr("Không rõ tên", "Unknown title")
    } else {
        &track.title
    }
}

fn display_artist(track: &Track) -> &str {
    if track.artist.trim().is_empty() {
        tr("Không rõ nghệ sĩ", "Unknown artist")
    } else {
        &track.artist
    }
}

fn format_time(ms: f64) -> String {
    let total = (ms.max(0.0) / 1000.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("puw-player");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(windows)]
fn system_dark() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let value: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut core::ffi::c_void,
            &mut size,
        )
    };
    status == 0 && data == 0
}

#[cfg(not(windows))]
fn system_dark() -> bool {
    false
}

#[cfg(windows)]
fn animations_enabled() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut enabled: i32 = 1;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut i32 as *mut core::ffi::c_void,
            0,
        )
    };
    ok != 0 && enabled != 0
}

#[cfg(not(windows))]
fn animations_enabled() -> bool {
    true
}

#[cfg(windows)]
fn window_hwnd(window: &AppWindow) -> Option<*mut core::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window().window_handle();
    let handle = handle.window_handle().ok()?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return None;
    };
    Some(win32.hwnd.get() as *mut core::ffi::c_void)
}

#[cfg(not(windows))]
fn window_hwnd(_window: &AppWindow) -> Option<*mut core::ffi::c_void> {
    None
}

#[cfg(windows)]
fn apply_border_style(window: &AppWindow, maximized: bool) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    let Some(hwnd) = window_hwnd(window) else {
        return;
    };
    let preference: i32 = if maximized {
        DWMWCP_DONOTROUND
    } else {
        DWMWCP_ROUND
    };
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &preference as *const i32 as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn apply_border_style(_window: &AppWindow, _maximized: bool) {}
