//! Interface language: Vietnamese (default) or English, switched in Settings.
//! `tr("tiếng Việt", "English")` picks the text for the current language (same scheme as PUW Notch).

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set_english(on: bool) {
    ENGLISH.store(on, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Text for the current language.
pub fn tr(vi: &'static str, en: &'static str) -> &'static str {
    if english() { en } else { vi }
}
