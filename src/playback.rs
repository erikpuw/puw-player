//! Audio transport: pure engine state, a Rodio output adapter, and a worker thread.

use crate::i18n::tr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::tag::Accessor;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackInfo {
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub codec: String,
    pub lossless: bool,
    /// Audio bitrate; for lossy files this is the quality figure people know (128/256/320).
    pub bitrate_kbps: u32,
}

/// Quality badge shown after the sample rate, using the usual store tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// Lossless above CD quality: 24-bit or more than 48 kHz.
    HiRes,
    Lossless,
    Lossy {
        kbps: u32,
    },
}

/// Spatial release named in the title or file name. Such files are usually a stereo render
/// (binaural = mixed for headphones); the label only says what the release is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpatialMix {
    Atmos,
    Binaural,
    BinauralAtmos,
}

impl SpatialMix {
    /// Whole words only, so "Atmosphere" is not Atmos.
    pub fn detect(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        let has = |word: &str| {
            lower
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| w == word)
        };
        match (has("atmos"), has("binaural")) {
            (true, true) => Some(Self::BinauralAtmos),
            (true, false) => Some(Self::Atmos),
            (false, true) => Some(Self::Binaural),
            (false, false) => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Atmos => "Atmos",
            Self::Binaural => "Binaural",
            Self::BinauralAtmos => "Binaural Atmos",
        }
    }
}

impl TrackInfo {
    pub fn quality(&self) -> Quality {
        if !self.lossless {
            Quality::Lossy {
                kbps: self.bitrate_kbps,
            }
        } else if self.bits >= 24 || self.sample_rate > 48_000 {
            Quality::HiRes
        } else {
            Quality::Lossless
        }
    }

    pub fn duration_ms(&self) -> u64 {
        (self.duration * 1000.0).round() as u64
    }
}

pub fn inspect_audio(path: &Path) -> Result<TrackInfo, String> {
    let tagged = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    let properties = tagged.properties();
    let file_type = tagged.file_type();
    let bits = properties.bit_depth().map(u16::from).unwrap_or(0);
    // An .m4a holds either AAC (lossy, no bit depth) or ALAC (lossless, has a bit depth);
    // name the codec, not the container.
    let alac = file_type == FileType::Mp4 && bits > 0;
    let codec = match file_type {
        FileType::Mp4 if alac => "alac",
        FileType::Mp4 => "aac",
        other => codec_name(other),
    };
    Ok(TrackInfo {
        duration: properties.duration().as_secs_f64(),
        sample_rate: properties.sample_rate().unwrap_or(0),
        channels: properties.channels().map(u16::from).unwrap_or(0),
        bits,
        codec: codec.to_owned(),
        lossless: alac || is_lossless(file_type),
        bitrate_kbps: properties.audio_bitrate().unwrap_or(0),
    })
}

/// Tag fields used by the library and the now-playing panel.
#[derive(Clone, Debug, Default)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork: Option<Vec<u8>>,
}

pub fn read_tags(path: &Path) -> Tags {
    let Some(tagged) = lofty::read_from_path(path).ok() else {
        return Tags::default();
    };
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let Some(tag) = tag else {
        return Tags::default();
    };
    Tags {
        title: tag.title().map(|t| t.to_string()).unwrap_or_default(),
        artist: tag.artist().map(|t| t.to_string()).unwrap_or_default(),
        album: tag.album().map(|t| t.to_string()).unwrap_or_default(),
        artwork: tag.pictures().first().map(|p| p.data().to_vec()),
    }
}

fn codec_name(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Aac => "aac",
        FileType::Aiff => "aiff",
        FileType::Ape => "ape",
        FileType::Flac => "flac",
        FileType::Mpeg => "mp3",
        FileType::Mpc => "mpc",
        FileType::Opus => "opus",
        FileType::Vorbis => "vorbis",
        FileType::Speex => "speex",
        FileType::Wav => "wav",
        FileType::WavPack => "wavpack",
        _ => "unknown",
    }
}

fn is_lossless(file_type: FileType) -> bool {
    matches!(
        file_type,
        FileType::Aiff | FileType::Ape | FileType::Flac | FileType::Wav | FileType::WavPack
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Idle,
    Playing,
    Paused,
    Stopped,
    Ended,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackCommand {
    Load {
        track_id: i64,
        path: PathBuf,
        generation: u64,
    },
    Play,
    Pause,
    Stop,
    Seek(u64),
    Volume(f32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaybackSnapshot {
    pub track_id: Option<i64>,
    pub generation: u64,
    pub state: PlaybackState,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub error: Option<String>,
}

pub trait AudioOutput: Send {
    fn load(&mut self, path: &Path) -> Result<u64, String>;
    fn play(&mut self);
    fn pause(&mut self);
    fn stop(&mut self);
    fn seek(&mut self, position_ms: u64) -> Result<(), String>;
    fn position_ms(&self) -> u64;
    fn finished(&self) -> bool;
    fn set_volume(&mut self, volume: f32);
    fn shutdown(&mut self) {
        self.stop();
    }
}

/// Transport logic with no audio dependency. The worker thread owns one and feeds it commands.
pub struct Engine<O: AudioOutput> {
    output: O,
    snapshot: PlaybackSnapshot,
}

impl<O: AudioOutput> Engine<O> {
    pub fn new(output: O) -> Self {
        Self {
            output,
            snapshot: PlaybackSnapshot {
                volume: 1.0,
                ..Default::default()
            },
        }
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot.clone()
    }

    #[doc(hidden)]
    pub fn output_mut(&mut self) -> &mut O {
        &mut self.output
    }

    pub fn handle(&mut self, command: PlaybackCommand) {
        match command {
            PlaybackCommand::Load {
                track_id,
                path,
                generation,
            } => self.load(track_id, generation, &path),
            PlaybackCommand::Play => self.play(),
            PlaybackCommand::Pause => self.pause(),
            PlaybackCommand::Stop => self.stop(),
            PlaybackCommand::Seek(position_ms) => self.seek(position_ms),
            PlaybackCommand::Volume(volume) => self.set_volume(volume),
        }
    }

    fn load(&mut self, track_id: i64, generation: u64, path: &Path) {
        if generation < self.snapshot.generation {
            return;
        }
        self.snapshot.generation = generation;
        self.snapshot.track_id = Some(track_id);
        self.snapshot.state = PlaybackState::Playing;
        self.snapshot.position_ms = 0;
        self.snapshot.duration_ms = 0;
        self.snapshot.error = None;
        match self.output.load(path) {
            Ok(duration_ms) => {
                self.snapshot.duration_ms = duration_ms;
                self.output.set_volume(self.snapshot.volume);
                self.output.play();
            }
            Err(message) => {
                self.snapshot.state = PlaybackState::Error;
                self.snapshot.error = Some(message);
            }
        }
    }

    fn play(&mut self) {
        match self.snapshot.state {
            PlaybackState::Paused | PlaybackState::Stopped => {
                self.output.play();
                self.snapshot.state = PlaybackState::Playing;
            }
            PlaybackState::Ended => {
                if let Ok(()) = self.output.seek(0) {
                    self.output.play();
                    self.snapshot.position_ms = 0;
                    self.snapshot.state = PlaybackState::Playing;
                }
            }
            _ => {}
        }
    }

    fn pause(&mut self) {
        if self.snapshot.state != PlaybackState::Playing {
            return;
        }
        self.snapshot.position_ms = self.clamped_position();
        self.output.pause();
        self.snapshot.state = PlaybackState::Paused;
    }

    fn stop(&mut self) {
        self.output.stop();
        self.snapshot.position_ms = 0;
        self.snapshot.state = PlaybackState::Stopped;
    }

    fn seek(&mut self, position_ms: u64) {
        if self.snapshot.track_id.is_none() {
            return;
        }
        let target = if self.snapshot.duration_ms > 0 {
            position_ms.min(self.snapshot.duration_ms)
        } else {
            0
        };
        match self.output.seek(target) {
            Ok(()) => {
                self.snapshot.position_ms = target;
                self.snapshot.error = None;
                if self.snapshot.state == PlaybackState::Ended {
                    self.snapshot.state = PlaybackState::Paused;
                }
            }
            Err(message) => self.snapshot.error = Some(message),
        }
    }

    fn set_volume(&mut self, volume: f32) {
        let volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            self.snapshot.volume
        };
        self.snapshot.volume = volume;
        self.output.set_volume(volume);
    }

    fn clamped_position(&self) -> u64 {
        let position = self.output.position_ms();
        if self.snapshot.duration_ms > 0 {
            position.min(self.snapshot.duration_ms)
        } else {
            position
        }
    }

    /// Refresh the position while playing and report the end of the track exactly once.
    pub fn tick(&mut self) {
        if self.snapshot.state != PlaybackState::Playing {
            return;
        }
        if self.output.finished() {
            self.snapshot.position_ms = self.snapshot.duration_ms;
            self.snapshot.state = PlaybackState::Ended;
        } else {
            self.snapshot.position_ms = self.clamped_position();
        }
    }

    pub fn shutdown(&mut self) {
        self.output.shutdown();
    }
}

pub struct RodioOutput {
    stream: Option<rodio::OutputStream>,
    sink: Option<rodio::Sink>,
    device: Option<(u32, u16)>,
    path: Option<PathBuf>,
    duration_ms: u64,
    volume: f32,
}

impl RodioOutput {
    pub fn new() -> Result<Self, String> {
        // Query device format without starting the WASAPI stream at app launch,
        // preventing startup DAC wake/DC-offset pops.
        let builder = rodio::OutputStreamBuilder::from_default_device().map_err(|e| {
            format!(
                "{} {e}",
                tr(
                    "Không mở được thiết bị âm thanh:",
                    "Could not open the audio device:"
                )
            )
        })?;
        use rodio::cpal::traits::{DeviceTrait, HostTrait};
        let device = rodio::cpal::default_host()
            .default_output_device()
            .and_then(|d| d.default_output_config().ok())
            .map(|cfg| (cfg.sample_rate().0, cfg.channels()));
        let _ = builder;
        Ok(Self {
            stream: None,
            sink: None,
            device,
            path: None,
            duration_ms: 0,
            volume: 1.0,
        })
    }

    fn ensure_stream(&mut self) -> Result<&rodio::OutputStream, String> {
        if self.stream.is_none() {
            let mut stream = rodio::OutputStreamBuilder::open_default_stream().map_err(|e| {
                format!(
                    "{} {e}",
                    tr(
                        "Không mở được thiết bị âm thanh:",
                        "Could not open the audio device:"
                    )
                )
            })?;
            stream.log_on_drop(false);
            let cfg = stream.config();
            self.device = Some((cfg.sample_rate(), cfg.channel_count()));
            // Keep a silent Zero source attached so the mixer stays warm with continuous 0.0 samples.
            stream
                .mixer()
                .add(rodio::source::Zero::new(cfg.channel_count(), cfg.sample_rate()));
            std::thread::sleep(Duration::from_millis(15));
            self.stream = Some(stream);
        }
        Ok(self.stream.as_ref().expect("stream initialized"))
    }

    fn silence_and_drop_sink(&mut self) {
        if let Some(sink) = self.sink.take() {
            if !sink.empty() && !sink.is_paused() {
                let current = sink.volume();
                sink.set_volume(current * 0.3);
                std::thread::sleep(Duration::from_millis(8));
                sink.set_volume(0.0);
                std::thread::sleep(Duration::from_millis(20));
            } else {
                sink.set_volume(0.0);
            }
            sink.stop();
        }
    }

    fn open_decoder(
        path: &Path,
    ) -> Result<rodio::Decoder<std::io::BufReader<std::fs::File>>, String> {
        let file = std::fs::File::open(path)
            .map_err(|e| format!("{} {e}", tr("Không mở được tệp:", "Could not open file:")))?;
        let byte_len = file.metadata().ok().map(|m| m.len());
        let mut builder = rodio::Decoder::builder()
            .with_data(std::io::BufReader::new(file))
            .with_seekable(true);
        if let Some(len) = byte_len {
            builder = builder.with_byte_len(len);
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            builder = builder.with_hint(ext);
        }
        builder
            .build()
            .map_err(|e| format!("{} {e}", tr("Không giải mã được:", "Could not decode:")))
    }
}

impl AudioOutput for RodioOutput {
    fn load(&mut self, path: &Path) -> Result<u64, String> {
        let decoder = Self::open_decoder(path)?;
        use rodio::Source;
        let duration_ms = decoder
            .total_duration()
            .map(|d| d.as_millis() as u64)
            .filter(|ms| *ms > 0)
            .or_else(|| inspect_audio(path).ok().map(|t| t.duration_ms()))
            .unwrap_or(0);
        self.silence_and_drop_sink();
        let volume = self.volume;
        let stream = self.ensure_stream()?;
        let sink = rodio::Sink::connect_new(stream.mixer());
        sink.pause();
        sink.set_volume(volume);
        sink.append(decoder.fade_in(Duration::from_millis(25)));
        self.sink = Some(sink);
        self.path = Some(path.to_path_buf());
        self.duration_ms = duration_ms;
        Ok(duration_ms)
    }
    fn play(&mut self) {
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
            sink.play();
        }
    }
    fn pause(&mut self) {
        if let Some(sink) = &self.sink {
            sink.pause();
        }
    }
    fn stop(&mut self) {
        self.silence_and_drop_sink();
    }
    fn seek(&mut self, position_ms: u64) -> Result<(), String> {
        // Keep a small margin before EOF so Symphonia does not reject seeks right at the end.
        let safe_ms = if self.duration_ms > 50 {
            position_ms.min(self.duration_ms.saturating_sub(50))
        } else {
            position_ms
        };
        let target = Duration::from_millis(safe_ms);
        if self.sink.as_ref().is_none_or(|s| s.empty())
            && let Some(path) = self.path.clone()
        {
            use rodio::Source;
            let decoder = Self::open_decoder(&path)?;
            let volume = self.volume;
            let stream = self.ensure_stream()?;
            let sink = rodio::Sink::connect_new(stream.mixer());
            sink.pause();
            sink.set_volume(volume);
            sink.append(decoder.fade_in(Duration::from_millis(20)));
            self.sink = Some(sink);
        }
        if let Some(sink) = &self.sink
            && sink.try_seek(target).is_ok()
        {
            return Ok(());
        }
        // Fallback: if the stream could not seek in-place, reopen and seek forward from start.
        if let Some(path) = self.path.clone() {
            use rodio::Source;
            let paused = self.sink.as_ref().is_none_or(|s| s.is_paused());
            self.silence_and_drop_sink();
            let volume = self.volume;
            let decoder = Self::open_decoder(&path)?;
            let stream = self.ensure_stream()?;
            let sink = rodio::Sink::connect_new(stream.mixer());
            sink.set_volume(volume);
            if paused {
                sink.pause();
            }
            sink.append(decoder.fade_in(Duration::from_millis(20)));
            let res = sink
                .try_seek(target)
                .map_err(|e| format!("{} {e}", tr("Không tua được:", "Could not seek:")));
            self.sink = Some(sink);
            return res;
        }
        Err(tr("Không tua được", "Could not seek").into())
    }
    fn position_ms(&self) -> u64 {
        self.sink
            .as_ref()
            .map(|s| s.get_pos().as_millis() as u64)
            .unwrap_or(0)
    }
    fn finished(&self) -> bool {
        self.sink.as_ref().is_none_or(|s| s.empty())
    }
    fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        if let Some(sink) = &self.sink {
            sink.set_volume(volume);
        }
    }
    fn shutdown(&mut self) {
        self.silence_and_drop_sink();
        if self.stream.is_some() {
            std::thread::sleep(Duration::from_millis(25));
            self.stream = None;
        }
    }
}

/// Output used when the machine has no audio device, so the UI can report it and recover.
pub struct SilentOutput;

impl AudioOutput for SilentOutput {
    fn load(&mut self, _path: &Path) -> Result<u64, String> {
        Err(tr(
            "Không có thiết bị âm thanh khả dụng",
            "No audio device available",
        )
        .into())
    }
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn stop(&mut self) {}
    fn seek(&mut self, _position_ms: u64) -> Result<(), String> {
        Ok(())
    }
    fn position_ms(&self) -> u64 {
        0
    }
    fn finished(&self) -> bool {
        false
    }
    fn set_volume(&mut self, _volume: f32) {}
}

enum Message {
    Command(PlaybackCommand),
    Shutdown,
}

/// Handle to the audio worker thread.
pub struct Player {
    tx: Sender<Message>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    worker: Option<std::thread::JoinHandle<()>>,
    device: Option<(u32, u16)>,
}

impl Player {
    pub fn start() -> Self {
        match RodioOutput::new() {
            Ok(output) => {
                let device = output.device;
                let mut player = Self::start_with(output);
                player.device = device;
                player
            }
            Err(_) => Self::start_with(SilentOutput),
        }
    }

    /// Output device format (sample rate in Hz, channels), kept separate from per-file
    /// properties. Numbers, not text, so the UI can format them in the current language.
    pub fn device(&self) -> Option<(u32, u16)> {
        self.device
    }

    pub fn start_with<O: AudioOutput + 'static>(output: O) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let snapshot = Arc::new(Mutex::new(PlaybackSnapshot {
            volume: 1.0,
            ..Default::default()
        }));
        let shared = Arc::clone(&snapshot);
        let worker = std::thread::spawn(move || run_worker(output, rx, shared));
        Self {
            tx,
            snapshot,
            worker: Some(worker),
            device: None,
        }
    }

    pub fn load(&self, track_id: i64, generation: u64, path: PathBuf) {
        self.send(PlaybackCommand::Load {
            track_id,
            path,
            generation,
        });
    }

    pub fn send(&self, command: PlaybackCommand) {
        self.send_message(Message::Command(command));
    }

    fn send_message(&self, message: Message) {
        let _ = self.tx.send(message);
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.tx.send(Message::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker<O: AudioOutput>(
    output: O,
    rx: Receiver<Message>,
    shared: Arc<Mutex<PlaybackSnapshot>>,
) {
    let mut engine = Engine::new(output);
    loop {
        let mut batch = Vec::new();
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Message::Command(command)) => batch.push(command),
            Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                engine.shutdown();
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Message::Command(cmd) => {
                    // Coalesce back-to-back Seek or Volume commands so dragging never floods the decoder.
                    match (batch.last_mut(), &cmd) {
                        (Some(PlaybackCommand::Seek(prev)), PlaybackCommand::Seek(next)) => {
                            *prev = *next;
                        }
                        (Some(PlaybackCommand::Volume(prev)), PlaybackCommand::Volume(next)) => {
                            *prev = *next;
                        }
                        _ => batch.push(cmd),
                    }
                }
                Message::Shutdown => {
                    engine.shutdown();
                    return;
                }
            }
        }
        for command in batch {
            engine.handle(command);
        }
        engine.tick();
        if let Ok(mut slot) = shared.lock() {
            *slot = engine.snapshot();
        }
    }
}
