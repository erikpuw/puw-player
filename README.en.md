<p align="center"><img src="assets/puw_1024.png" width="96" alt="Puw Player"></p>

<h1 align="center">Puw Player</h1>

<p align="center">An offline music player for Windows · <a href="README.md">Tiếng Việt</a></p>

Puw Player plays **music that is already on your computer**. No streaming, no account, no subscription, no ties to Spotify, Apple Music or any other platform. The only network use is looking up **lyrics** for songs you already have (LRCLIB, off by default).

## Features

| Area | What you get |
|---|---|
| Library | Add files or whole folders. Only paths are stored; your files are never copied, moved or deleted. |
| Playback | Play/pause, stop, previous/next, ±10 s seek, draggable timeline, volume. |
| Queue | Shuffle without repeats, Previous returns to the song you just heard. Repeat: off / all / one. |
| Playlists | Create, rename, delete. Add songs from the Songs tab, remove songs from a playlist. Deleting a playlist never deletes music files. |
| Quality | A badge after the source details: **Hi-Res** (lossless 24-bit or above 48 kHz), **Lossless** (FLAC, ALAC, WAV…), or the bitrate for compressed files (e.g. 320 kbps). |
| Cover flow | Current cover in the middle, two covers on each side from the queue, sliding when the track changes. |
| Lyrics | A `.lrc` file next to the song first, then saved lyrics, then LRCLIB if you turn it on. Synced to playback, click a line to seek, per-song ±0.5 s offset. Downloaded lyrics work offline. |
| Windows | Shows up in the Windows media flyout and answers keyboard media keys. "Now playing" readers such as PUW Notch can see it too. |
| Interface | System / light / dark theme, Vietnamese / English, respects Windows reduced-motion setting. |
| Shortcuts | `Space` play/pause · `←` `→` seek 10 s · `↑` `↓` volume · `Esc` close the open panel. |

## Privacy

| Sent over the network | When |
|---|---|
| Title, artist, album, duration | Only with "Fetch lyrics from LRCLIB" on, for songs without lyrics yet. |
| File paths, audio | Never. |

## Formats

| Format | Status |
|---|---|
| FLAC, WAV PCM, M4A (AAC-LC) | Played and checked on real files. |
| MP3 | Tags are read; playback not fully tested yet. |
| WMA, APE, DSD, Opus, HE-AAC | Not supported. |

## Build and run

Needs Windows 10/11 and [Rust](https://rustup.rs) 1.92 or newer (MSVC toolchain).

```bash
cargo build --release
```

The app is `target/release/puw-player.exe`. The library starts empty; add music with the **+** button at the top. Data (library, playlists, settings, saved lyrics) lives in `%APPDATA%\puw-player\library.db`.

## Development

```bash
cargo test --no-default-features     # core tests, no audio device needed
cargo clippy --all-targets -- -D warnings
```

Rust + [Slint](https://slint.dev) for the interface, Rodio/Symphonia for audio, SQLite for data. The icon is drawn by `assets/make_icon.py`.

## Limitations

- No drag-and-drop into the window yet; add music with the **+** button.
- Artwork comes only from images embedded in the file; `cover.jpg` in the folder is not read yet.
- Windows Snap Layouts and 125%/150% DPI displays have not been tested thoroughly.
