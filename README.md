<div align="center">

<img src="packaging/icons/tubify-256.png" width="128" alt="Tubify icon">

# Tubify

a YouTube Music player for desktop, built with Qt Quick on top of a Rust core

[![qt](https://img.shields.io/badge/Qt_Quick-6.10-41cd52?style=for-the-badge&logo=qt&logoColor=white&labelColor=1d1b20)](https://www.qt.io)
[![rust](https://img.shields.io/badge/core-rust-dea584?style=for-the-badge&logo=rust&logoColor=white&labelColor=1d1b20)](ytmusic-core)
[![license](https://img.shields.io/github/license/swiftify-org/swiftify?style=for-the-badge&labelColor=1d1b20)](LICENSE)

</div>

## about

Tubify is a fork of [Swiftify](https://github.com/swiftify-org/swiftify) with
the whole interface kept 1:1, moved from Spotify to **YouTube Music**. The
Material 3 Expressive shell, the immersive player, synced lyrics, visualizers,
the equalizer, local files, stats, Discord presence and MPRIS are all still
there.

What changed is underneath:

- one sign-in: the `Cookie` header from a signed-in `music.youtube.com`
- the library, liked songs, history, playlists, albums, artists and search come
  from YouTube Music's own API
- songs are fetched with `yt-dlp` into a local cache and played from disk
  through the same equalizer, analyser and output local files use
- **DNS resolver** in Settings: filters sponsored shelves and promoted placements
  from every YouTube Music answer. Songs themselves are always fetched cleanly.

the UI is not a webview: it is Qt Quick, and the core is Rust.

Tubify is unofficial and is not affiliated with YouTube or Google.

## requirements

- Qt **6.10 or newer** (Core, Gui, Network, Concurrent, Qml, Quick,
  QuickControls2, ShaderTools, DBus on Linux)
- CMake 3.24+, a C++20 compiler, Rust 1.85+
- [QCoro](https://github.com/qcoro/qcoro) 0.10+ (fetched automatically if it is
  not installed)
- `yt-dlp` and `ffmpeg` on `PATH` for playback
- `libpulse` on Linux (the audio-device monitor), ALSA for the core
- the Material Symbols and Google Sans Flex fonts are downloaded at configure
  time (or turned off with `-DSWIFTIFY_DOWNLOAD_ICON_FONT=OFF` and
  `-DSWIFTIFY_DOWNLOAD_INTERFACE_FONT=OFF`)

## building

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j"$(nproc)"
./build/swiftify
```

If Qt or QCoro live in a custom prefix:

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$HOME/.local"
```

On Arch:

```sh
sudo pacman -S qt6-base qt6-declarative qt6-shadertools qt6-tools \
  alsa-lib libpulse cmake ninja rust yt-dlp ffmpeg
```

## signing in

1. open [music.youtube.com](https://music.youtube.com) and sign in
2. press `F12`, open **Network**, reload the page
3. click any request to `music.youtube.com`
4. copy the whole **Cookie** request header
5. paste it into Tubify's setup screen and choose **Sign In**

The cookie is kept on this computer in `~/.local/share/Tubify/login.json`,
readable only by its owner. Nothing is sent anywhere except to YouTube Music.

## features

- home with continue listening, recently played and your playlists
- search across songs, albums, artists and playlists
- liked songs, playlists, albums and artist pages
- playlists: create, rename, describe, add, remove, reorder and delete
- synced lyrics from LRCLIB, with YouTube Music's own lyrics as a fallback
- immersive player, queue, sleep timer and crossfade
- equalizer, tempo/pitch, preamp and volume normalization
- visualizers and the living backdrop
- local files with tag editing and local playlists
- listening stats from stats.fm, Last.fm, or the plays kept on this computer
- Discord rich presence and MPRIS
- DNS resolver in Settings

## notes

- YouTube Music does not let an app set a playlist's own cover, so that one
  action reports that instead of pretending to work.
- an artist's genres are not published by YouTube Music; the stats page shows
  the artist without a genre instead of guessing.
- playback needs `yt-dlp` and `ffmpeg`; without them the app still browses the
  library but cannot fetch a song.
- Last.fm stats need a free API key from last.fm/api/account/create. The key and
  username stay on this computer.

## credits

Tubify is a fork of [Swiftify](https://github.com/swiftify-org/swiftify) by
shred0s, which is GPL-3.0. The Material 3 tokens, motion and shapes come from
end4. The Rust core keeps Swiftify's audio pipeline, equalizer, analyser and
local-file playback.

## license

GPL-3.0. See [LICENSE](LICENSE).
