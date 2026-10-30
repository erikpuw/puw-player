use puw_player::playback::{Quality, TrackInfo, inspect_audio};

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
    assert_eq!(t.quality(), Quality::Lossless);
    assert!(inspect_audio(&dir.path().join("missing.wav")).is_err());
    std::fs::write(dir.path().join("bad.mp3"), b"not music").unwrap();
    assert!(inspect_audio(&dir.path().join("bad.mp3")).is_err());
}

#[test]
fn quality_tiers_follow_bit_depth_rate_and_codec() {
    let info = |lossless, bits, sample_rate, bitrate_kbps| TrackInfo {
        lossless,
        bits,
        sample_rate,
        bitrate_kbps,
        ..Default::default()
    };
    assert_eq!(info(true, 24, 96_000, 4_000).quality(), Quality::HiRes);
    assert_eq!(info(true, 24, 44_100, 2_000).quality(), Quality::HiRes);
    assert_eq!(info(true, 16, 88_200, 2_000).quality(), Quality::HiRes);
    assert_eq!(info(true, 16, 44_100, 1_000).quality(), Quality::Lossless);
    assert_eq!(
        info(false, 0, 44_100, 256).quality(),
        Quality::Lossy { kbps: 256 }
    );
}
