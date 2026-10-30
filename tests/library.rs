use puw_player::library::{Library, Track};
use puw_player::settings::{Settings, Theme};

fn sample(path: &std::path::Path) -> Track {
    Track {
        path: path.to_path_buf(),
        title: "Nhạc thử".into(),
        ..Default::default()
    }
}

#[test]
fn persistence_deduplicates_import_and_playlist_delete_keeps_file() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("bài hát.wav");
    std::fs::write(&music, b"original").unwrap();
    let db = dir.path().join("library.db");
    let mut lib = Library::open(&db).unwrap();
    let first = lib.upsert_track(&sample(&music)).unwrap();
    assert_eq!(lib.upsert_track(&sample(&music)).unwrap(), first);
    assert_eq!(lib.tracks(None).unwrap().len(), 1);
    let p = lib.create_playlist("Hay nghe").unwrap();
    lib.add_to_playlist(p, first).unwrap();
    lib.add_to_playlist(p, first).unwrap();
    assert_eq!(lib.tracks(Some(p)).unwrap().len(), 1);
    drop(lib);
    let mut lib = Library::open(&db).unwrap();
    assert_eq!(lib.tracks(Some(p)).unwrap()[0].id, first);
    lib.delete_playlist(p).unwrap();
    assert_eq!(std::fs::read(&music).unwrap(), b"original");
    assert_eq!(lib.tracks(None).unwrap().len(), 1);
}

#[test]
fn playlist_order_survives_reopen_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(&dir.path().join("lib.db")).unwrap();
    let a = lib
        .upsert_track(&sample(&dir.path().join("a.flac")))
        .unwrap();
    let b = lib
        .upsert_track(&sample(&dir.path().join("b.flac")))
        .unwrap();
    let p = lib.create_playlist("Test").unwrap();
    lib.add_to_playlist(p, b).unwrap();
    lib.add_to_playlist(p, a).unwrap();
    assert_eq!(
        lib.tracks(Some(p))
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>(),
        vec![b, a]
    );
    lib.remove_from_playlist(p, b).unwrap();
    assert_eq!(lib.tracks(Some(p)).unwrap()[0].id, a);
    assert!(lib.create_playlist("  ").is_err());
}

#[test]
fn settings_and_lyric_offset_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("lib.db");
    let mut lib = Library::open(&db).unwrap();
    assert_eq!(lib.settings().unwrap(), Settings::default());
    let id = lib
        .upsert_track(&sample(&dir.path().join("a.flac")))
        .unwrap();
    lib.save_settings(&Settings {
        theme: Theme::Dark,
        lyrics_online: true,
    })
    .unwrap();
    lib.set_lyric_offset(id, 250).unwrap();
    drop(lib);
    let lib = Library::open(&db).unwrap();
    assert_eq!(
        lib.settings().unwrap(),
        Settings {
            theme: Theme::Dark,
            lyrics_online: true
        }
    );
    assert_eq!(lib.lyric_offset(id).unwrap(), 250);
    assert_eq!(lib.lyric_offset(id + 99).unwrap(), 0);
}

#[test]
fn import_skips_duplicates_and_reports_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("Album");
    std::fs::create_dir(&nested).unwrap();
    for name in ["một.flac", "hai.mp3"] {
        std::fs::write(nested.join(name), b"silence").unwrap();
    }
    std::fs::write(dir.path().join("ignore.txt"), b"notes").unwrap();
    let mut lib = Library::open(&dir.path().join("lib.db")).unwrap();
    let report = lib.import_folder(&nested);
    assert_eq!(report.imported, 0);
    assert_eq!(
        report.failed.len(),
        2,
        "corrupt files are reported, not silently dropped"
    );
    assert_eq!(lib.tracks(None).unwrap().len(), 0);
    std::fs::remove_file(nested.join("hai.mp3")).unwrap();
    assert!(lib.import_file(&nested.join("hai.mp3")).is_err());
}

#[test]
fn library_is_the_lyrics_store() {
    use puw_player::lyrics::LyricsStore;
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(&dir.path().join("lib.db")).unwrap();
    let id = lib
        .upsert_track(&sample(&dir.path().join("a.flac")))
        .unwrap();
    assert!(LyricsStore::cached(&lib, id).is_none());
    lib.store(id, "lrclib-synced", "[00:01.00]Một câu");
    assert_eq!(lib.cached(id).unwrap().0, "lrclib-synced");
    assert_eq!(lib.cached(id).unwrap().1, "[00:01.00]Một câu");
}
