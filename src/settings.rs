
use crate::queue::RepeatMode;
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

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub theme: Theme,
    pub lyrics_online: bool,
    pub english: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            lyrics_online: false,
            english: false,
            shuffle: false,
            repeat: RepeatMode::default(),
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
            "shuffle" => settings.shuffle = value == "1",
            "repeat" => settings.repeat = repeat_from(&value),
            _ => {}
        }
    }
    Ok(settings)
}

fn repeat_key(mode: RepeatMode) -> &'static str {
    match mode {
        RepeatMode::Off => "off",
        RepeatMode::All => "all",
        RepeatMode::One => "one",
    }
}

fn repeat_from(value: &str) -> RepeatMode {
    match value {
        "all" => RepeatMode::All,
        "one" => RepeatMode::One,
        _ => RepeatMode::Off,
    }
}

fn put(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save(conn: &Connection, settings: &Settings) -> Result<(), String> {
    put(conn, "theme", settings.theme.as_str())?;
    put(
        conn,
        "lyrics_online",
        if settings.lyrics_online { "1" } else { "0" },
    )?;
    put(conn, "language", if settings.english { "en" } else { "vi" })?;
    put(conn, "shuffle", if settings.shuffle { "1" } else { "0" })?;
    put(conn, "repeat", repeat_key(settings.repeat))?;
    Ok(())
}
