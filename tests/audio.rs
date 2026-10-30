use puw_player::playback::inspect_audio;

#[test]
fn wav_metadata_and_decode_are_real() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Âm thanh.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for _ in 0..88200 {
        writer.write_sample(0_i16).unwrap();
    }
    writer.finalize().unwrap();
    let t = inspect_audio(&path).unwrap();
    assert!((t.duration - 1.0).abs() < 0.01);
    assert_eq!(t.sample_rate, 44100);
    assert_eq!(t.channels, 2);
    assert_eq!(t.bits, 16);
    assert!(inspect_audio(&dir.path().join("missing.wav")).is_err());
    std::fs::write(dir.path().join("bad.mp3"), b"not music").unwrap();
    assert!(inspect_audio(&dir.path().join("bad.mp3")).is_err());
}
