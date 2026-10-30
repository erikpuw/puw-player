use puw_player::{
    artwork::fit_cover,
    i18n,
    lyrics::{active_line, parse_lrc},
    queue::{Queue, RepeatMode},
};

#[test]
fn cover_preserves_aspect_and_never_upscales() {
    assert_eq!(fit_cover(400, 200, 256), Some((256, 128)));
    assert_eq!(fit_cover(200, 400, 256), Some((128, 256)));
    assert_eq!(fit_cover(100, 100, 256), Some((100, 100)));
    assert_eq!(fit_cover(0, 200, 256), None);
}

#[test]
fn queue_repeat_respects_selected_playlist() {
    let mut q = Queue::new(vec![11, 22], 0);
    assert_eq!(q.advance(false), Some(22));
    assert_eq!(q.advance(false), None);
    q.repeat = RepeatMode::All;
    assert_eq!(q.advance(false), Some(11));
    q.repeat = RepeatMode::One;
    assert_eq!(q.advance(false), Some(11));
    assert_eq!(q.advance(true), Some(22));
    assert_eq!(Queue::new(vec![], 9).advance(false), None);
}

#[test]
fn shuffle_has_no_duplicates_and_previous_uses_history() {
    let mut q = Queue::new(vec![1, 2, 3, 4], 0);
    q.set_shuffle(true);
    let mut played = vec![q.current().unwrap()];
    while let Some(id) = q.advance(false) {
        played.push(id);
    }
    assert_eq!(played.len(), 4);
    let mut unique = played.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4);
    assert_eq!(q.previous(), Some(played[2]));
    assert_eq!(q.advance(true), Some(played[3]));
}

#[test]
fn shuffle_from_middle_still_plays_every_track() {
    let mut q = Queue::new(vec![1, 2, 3, 4, 5], 3);
    q.set_shuffle(true);
    assert_eq!(q.current(), Some(4));
    let mut played = vec![4];
    while let Some(id) = q.advance(false) {
        played.push(id);
    }
    played.sort();
    assert_eq!(played, vec![1, 2, 3, 4, 5]);
}

#[test]
fn single_track_shuffle_and_repeat_are_bounded() {
    let mut q = Queue::new(vec![7], 0);
    q.set_shuffle(true);
    assert_eq!(q.advance(false), None);
    q.repeat = RepeatMode::All;
    assert_eq!(q.advance(false), Some(7));
}

#[test]
fn lyrics_parse_multiple_times_offset_and_malformed_lines() {
    let lines = parse_lrc(
        "[ar:Test]\n[offset:100]\n[00:01.00][00:03.5]Một câu\n[broken]Bỏ qua\n[00:02.00]Câu hai\n[00:03.50]Câu cùng lúc",
    );
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].time_ms, 1100);
    assert_eq!(lines[2].time_ms, 3600);
    assert_eq!(active_line(&lines, 1099, 0), None);
    assert_eq!(active_line(&lines, 1100, 0), Some(0));
    assert_eq!(active_line(&lines, 2200, -200), Some(0));
    assert_eq!(active_line(&lines, 3700, 0), Some(3));
    assert_eq!(active_line(&lines, 1200, 0), Some(0));
}

#[test]
fn interface_language_switches_text() {
    i18n::set_english(false);
    assert_eq!(i18n::tr("Nhạc", "Music"), "Nhạc");
    i18n::set_english(true);
    assert_eq!(i18n::tr("Nhạc", "Music"), "Music");
    i18n::set_english(false);
}

#[test]
fn removing_from_queue_keeps_the_current_song_in_place() {
    let mut q = Queue::new(vec![1, 2, 3, 4], 2);
    q.remove(1);
    assert_eq!(q.current(), Some(3));
    assert_eq!(q.neighbor(-1), Some(2));
    q.remove(4);
    assert_eq!(q.advance(true), None);
    q.remove(3);
    assert_eq!(q.ids(), &[2]);
    assert_eq!(q.current(), Some(2));
}
