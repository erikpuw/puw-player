//! Persisted preferences. Stored in the library database so one file carries all app state.

use rusqlite::{Connection, params};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
    fn from_str(value: &str) -> Self {
        match value {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }
}

/// Online lyric lookup is opt-in: the app must run fully offline by default.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub theme: Theme,
    pub lyrics_online: bool,
    /// Interface language: false = Vietnamese (default), true = English.
    pub english: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            lyrics_online: false,
            english: false,
        }
    }
}

pub fn load(conn: &Connection) -> Result<Settings, String> {
    let mut statement = conn
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut settings = Settings::default();
    for row in rows {
        let (key, value) = row.map_err(|e| e.to_string())?;
        match key.as_str() {
            "theme" => settings.theme = Theme::from_str(&value),
            "lyrics_online" => settings.lyrics_online = value == "1",
            "language" => settings.english = value == "en",
            _ => {}
        }
    }
    Ok(settings)
}

pub fn save(conn: &Connection, settings: &Settings) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES('theme', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![settings.theme.as_str()],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES('lyrics_online', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![if settings.lyrics_online { "1" } else { "0" }],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES('language', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![if settings.english { "en" } else { "vi" }],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
