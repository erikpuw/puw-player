use std::path::{Path, PathBuf};

use puw_player::playback::{AudioOutput, Engine, PlaybackCommand, PlaybackState};

#[derive(Default)]
struct FakeOutput {
    duration_ms: u64,
    position_ms: u64,
    paused: bool,
    finished: bool,
    fail_once: Option<String>,
    loaded: Vec<PathBuf>,
    volume: f32,
}

impl AudioOutput for FakeOutput {
    fn load(&mut self, path: &Path) -> Result<u64, String> {
        if let Some(message) = self.fail_once.take() {
            return Err(message);
        }
        self.loaded.push(path.to_path_buf());
        self.position_ms = 0;
        self.paused = false;
        self.finished = false;
        Ok(self.duration_ms)
    }
    fn play(&mut self) {
        self.paused = false;
    }
    fn pause(&mut self) {
        self.paused = true;
    }
    fn stop(&mut self) {
        self.paused = false;
        self.finished = false;
        self.position_ms = 0;
    }
    fn seek(&mut self, position_ms: u64) -> Result<(), String> {
        if let Some(message) = self.fail_once.take() {
            return Err(message);
        }
        self.position_ms = position_ms;
        Ok(())
    }
    fn position_ms(&self) -> u64 {
        self.position_ms
    }
    fn finished(&self) -> bool {
        self.finished
    }
    fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }
}

fn engine(duration_ms: u64) -> Engine<FakeOutput> {
    Engine::new(FakeOutput {
        duration_ms,
        volume: 1.0,
        ..Default::default()
    })
}

fn load(id: i64, generation: u64) -> PlaybackCommand {
    PlaybackCommand::Load {
        track_id: id,
        path: PathBuf::from(format!("{id}.flac")),
        generation,
    }
}

#[test]
fn load_reports_duration_and_starts_playing() {
    let mut engine = engine(93_000);
    engine.handle(load(7, 1));
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.track_id, Some(7));
    assert_eq!(snapshot.state, PlaybackState::Playing);
    assert_eq!(snapshot.duration_ms, 93_000);
    assert_eq!(snapshot.position_ms, 0);
    assert!(snapshot.error.is_none());
}

#[test]
fn stale_generation_never_overrides_newer_track() {
    let mut engine = engine(10_000);
    engine.handle(load(2, 5));
    engine.handle(load(1, 4));
    assert_eq!(engine.snapshot().track_id, Some(2));
    engine.output_mut().position_ms = 2_500;
    engine.tick();
    assert_eq!(engine.snapshot().position_ms, 2_500);
}

#[test]
fn seek_clamps_to_duration_and_stop_returns_to_start() {
    let mut engine = engine(90_000);
    engine.handle(load(1, 1));
    engine.handle(PlaybackCommand::Seek(120_000));
    assert_eq!(engine.snapshot().position_ms, 90_000);
    engine.handle(PlaybackCommand::Pause);
    engine.handle(PlaybackCommand::Seek(30_000));
    assert_eq!(engine.snapshot().position_ms, 30_000);
    assert_eq!(engine.snapshot().state, PlaybackState::Paused);
    engine.handle(PlaybackCommand::Stop);
    assert_eq!(engine.snapshot().position_ms, 0);
    assert_eq!(engine.snapshot().state, PlaybackState::Stopped);
    assert_eq!(engine.output_mut().position_ms, 0);
}

#[test]
fn paused_position_does_not_drift_and_finish_is_reported_once() {
    let mut engine = engine(5_000);
    engine.handle(load(1, 1));
    engine.output_mut().position_ms = 1_500;
    engine.tick();
    assert_eq!(engine.snapshot().position_ms, 1_500);
    engine.handle(PlaybackCommand::Pause);
    engine.tick();
    assert_eq!(engine.snapshot().position_ms, 1_500);
    engine.handle(PlaybackCommand::Play);
    engine.output_mut().finished = true;
    engine.tick();
    assert_eq!(engine.snapshot().state, PlaybackState::Ended);
    assert_eq!(engine.snapshot().position_ms, 5_000);
}

#[test]
fn output_failure_is_recoverable() {
    let mut engine = engine(1_000);
    engine.output_mut().fail_once = Some("no audio device".into());
    engine.handle(load(3, 1));
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.state, PlaybackState::Error);
    assert_eq!(snapshot.error.as_deref(), Some("no audio device"));
    assert_eq!(snapshot.track_id, Some(3));
    engine.handle(load(4, 2));
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.state, PlaybackState::Playing);
    assert!(snapshot.error.is_none());
    assert_eq!(snapshot.track_id, Some(4));
}

#[test]
fn seek_failure_keeps_transport_and_exposes_error() {
    let mut engine = engine(8_000);
    engine.handle(load(1, 1));
    engine.output_mut().position_ms = 2_000;
    engine.tick();
    engine.output_mut().fail_once = Some("seek unsupported".into());
    engine.handle(PlaybackCommand::Seek(6_000));
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.error.as_deref(), Some("seek unsupported"));
    assert_eq!(snapshot.position_ms, 2_000);
}

#[test]
fn volume_is_clamped() {
    let mut engine = engine(1_000);
    engine.handle(PlaybackCommand::Volume(1.7));
    assert_eq!(engine.snapshot().volume, 1.0);
    engine.handle(PlaybackCommand::Volume(-0.3));
    assert_eq!(engine.snapshot().volume, 0.0);
    assert_eq!(engine.output_mut().volume, 0.0);
}
