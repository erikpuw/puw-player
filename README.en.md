<p align="center"><img src="assets/puw_1024.png" width="96" alt="Puw Player"></p>

<h1 align="center">Puw Player</h1>

<p align="center">An offline music player for Windows · <a href="README.md">Tiếng Việt</a></p>

Puw Player plays music that is already on your computer. No streaming, no account, no subscription. The only network use is looking up lyrics (LRCLIB, off by default).

> **Download only from the author's source.** Puw Player is published by its author (erikpuw) only at [github.com/erikpuw/puw-player](https://github.com/erikpuw/puw-player). A copy from anywhere else (file-sharing sites, other repositories, repackaged builds, files sent over the internet) is not controlled by the author and comes with no guarantee: it may have been modified or had code added. Use only the build in this repository's [Releases](https://github.com/erikpuw/puw-player/releases).

## Features

- **Library:** add files or whole folders. The app only remembers paths and never copies, moves or deletes your music files. You can remove songs from the library, or clean up songs whose file is gone in Settings.
- **Playback:** play, pause, stop, previous and next, 10 second seek, a draggable timeline, volume.
- **Shuffle and repeat:** shuffle without repeats, repeat off, all or one.
- **Search:** a box on the Songs tab and inside each playlist filters by title or artist.
- **Playlists:** create, rename, delete, add and remove songs. Deleting a playlist never deletes music files.
- **Quality:** a Hi-Res Lossless, Lossless or bitrate badge sits next to the song details.
- **Lyrics:** taken from a `.lrc` file next to the song, saved lyrics, or LRCLIB if you turn it on. Lines follow the music, click one to seek, and each song has a ±0.5 s offset.
- **Windows:** shows in the Windows media flyout and answers media keys. "Now playing" readers such as PUW Notch can see the song and the shuffle and repeat state, and can control them back.
- **Interface:** system, light or dark theme, Vietnamese or English.
- **Shortcuts:** `Space` play or pause, `←` `→` seek, `↑` `↓` volume, `Esc` close the open panel.

## Screenshots

| | |
|---|---|
| ![Now playing](assets/screenshots/player-dark.png)<br>Now playing | ![Quality](assets/screenshots/player-quality.png)<br>Quality badge |
| ![Lyrics](assets/screenshots/player-lyrics.png)<br>Lyrics | ![Playlists](assets/screenshots/player-playlist.png)<br>Playlists |
| ![Settings](assets/screenshots/player-settings.png)<br>Settings | ![Light theme](assets/screenshots/player-light.png)<br>Light theme |

The screenshots use a self-made demo library (song names, artists, artwork and lyrics are all demo data) so no real recording's copyright is involved.

## Privacy

| Sent over the network | When |
|---|---|
| Title, artist, album, duration | Only with "Fetch lyrics from LRCLIB" on, for songs without lyrics yet. |
| File paths, audio | Never. |

## Formats

| Format | Status |
|---|---|
| FLAC, WAV, M4A (AAC) | Played and checked on real files. |
| MP3 | Tags are read, playback not fully tested yet. |
| WMA, APE, DSD, Opus, HE-AAC | Not supported. |

## Build and run

Needs Windows 10 or 11 and [Rust](https://rustup.rs) 1.92 or newer.

```bash
cargo build --release
```

The app is `target/release/puw-player.exe`. The library starts empty, add music with the **+** button at the top. Data lives in `%APPDATA%\puw-player\library.db`.

The exe is not code-signed, so nothing but the official source at the top of this page confirms a file is the author's. Windows SmartScreen or antivirus may warn. If you download the zip, right-click it, open Properties, tick **Unblock**, then extract.

## Limitations

- No drag and drop into the window yet, add music with the **+** button.
- Artwork comes only from images embedded in the file, `cover.jpg` in the folder is not read yet.
- Windows Snap Layouts and 125%, 150% DPI displays have not been tested thoroughly.

## License

[MIT](LICENSE)
