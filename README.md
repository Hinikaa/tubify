<div align="center">

<img src="packaging/icons/tubify-256.png" width="128" alt="Tubify icon">

# Tubify

**YouTube Music, without the browser.**

A native desktop client built with Qt Quick and a Rust core.

[![qt](https://img.shields.io/badge/Qt_Quick-6.10-41cd52?style=for-the-badge&logo=qt&logoColor=white&labelColor=1d1b20)](https://www.qt.io)
[![rust](https://img.shields.io/badge/core-rust-dea584?style=for-the-badge&logo=rust&logoColor=white&labelColor=1d1b20)](ytmusic-core)
[![license](https://img.shields.io/badge/license-GPL--3.0-8f6cff?style=for-the-badge&labelColor=1d1b20)](LICENSE)
[![release](https://img.shields.io/github/v/release/Hinikaa/tubify?style=for-the-badge&color=6750a4&labelColor=1d1b20)](https://github.com/Hinikaa/tubify/releases/latest)

**[screenshots](#screenshots) · [install](#install) · [build](#build) · [first run](#first-run) · [last.fm](#lastfm) · [discord](#discord) · [limits](#notes-and-limits)**

<img src="assets/immersive-tubify.png" alt="Tubify immersive player">

</div>

## why another one

Most YouTube Music desktop apps are a webview in a trench coat. Tubify is not.
The interface is Qt Quick, the playback engine is Rust, and the only thing the
browser does is help you sign in.

It started as a fork of [Swiftify](https://github.com/swiftify-org/swiftify),
keeping its Material 3 Expressive shell and replacing the Spotify backend with
YouTube Music. The equalizer, immersive player, synced lyrics, visualizers,
local files, stats and queue are all still here.

There is no Spotify Premium requirement, no Spotify client ID, and no Spotify
account. A normal YouTube Music account is enough.

Tubify is unofficial and is not affiliated with YouTube or Google.

## screenshots

<details open>
<summary><b>Immersive player</b> · lyrics and visualizer</summary>
<br>

<table>
  <tr>
    <td><img src="assets/lyrics.png" alt="immersive player with synced lyrics and bar visualizer" width="480"></td>
    <td><img src="assets/visualizer.png" alt="immersive player with the bar visualizer" width="480"></td>
  </tr>
  <tr>
    <td align="center"><sub>synced lyrics, the line being sung lit up</sub></td>
    <td align="center"><sub>the cover, the seek line and the visualizer</sub></td>
  </tr>
</table>

</details>

<details>
<summary><b>Compact player</b> · the narrow layout</summary>
<br>

<img src="assets/player.png" alt="the player in its narrow layout" width="320">



## what you get

<details open>
<summary><b>Library and browsing</b></summary>
<br>

- Home, search, liked songs, playlists, albums and artist pages from your own
  YouTube Music account
- Playlist editing: create, rename, describe, add, remove, reorder, delete
- Local files with tag editing and local playlists
- Listening stats from stats.fm, Last.fm, or the plays kept on this computer

</details>

<details>
<summary><b>Playback and sound</b></summary>
<br>

- Immersive player, queue, sleep timer and crossfade
- Equalizer, tempo/pitch, preamp and volume normalization
- Bitrate and sample rate of the playing file, shown under the title
- Synced lyrics from LRCLIB, with YouTube Music's lyrics as a fallback
- Visualizers and the living backdrop

</details>

<details>
<summary><b>Integrations</b></summary>
<br>

- Last.fm scrobbling, connected from Settings
- Discord rich presence, with covers for local files too
- MPRIS
- A **DNS resolver** switch in Settings that keeps sponsored shelves and
  promoted placements out of the library answers

</details>

<details>
<summary><b>Artwork</b></summary>
<br>

The player and song rows use Deezer artwork when it is available, and fall
back to YouTube Music's own artwork. The Last.fm rows do the same.

</details>

## before you start

<details>
<summary><b>What you need</b></summary>
<br>

- **A YouTube Music account.** Free is fine. No Premium and no developer app.
- **A way to sign in.** The setup screen can read the login from Helium,
  Chromium, Chrome, Brave or Firefox. If that does not work, you can paste the
  `Cookie` header by hand.
- **`yt-dlp` and `ffmpeg`.** Playback downloads through `yt-dlp` and decodes
  with `ffmpeg`. Without them the app still browses your library, but it cannot
  fetch a song.
- **Optional: Last.fm.** Reading stats needs only a username. Scrobbling needs
  a one-time connect in Settings.

</details>

## install

Grab the latest build from the [releases page](https://github.com/Hinikaa/tubify/releases/latest).

<details>
<summary><b>Linux</b> · portable zip, tarball, or .deb</summary>
<br>

| file | how to run |
|---|---|
| `tubify-0.1.1-linux-x86_64.zip` | unzip anywhere, run `bin/tubify`. No install step. |
| `tubify-0.1.1-linux-x86_64.tar.gz` | extract anywhere, run `bin/tubify`. |
| `tubify_0.1.1_amd64.deb` | `sudo apt install ./tubify_0.1.1_amd64.deb` |

The Linux builds bundle Qt and most libraries. `yt-dlp` and `ffmpeg` are still
needed for playback; put them on `PATH` or next to the executable.

Building from source? The repo ships a `.desktop` file and icons in
`packaging/`. Install the binary somewhere on your `PATH` and copy
`packaging/tubify.desktop` to `~/.local/share/applications/`.

The audio core uses ALSA, and the device monitor uses PulseAudio. Almost every
desktop already has both.

</details>

<details>
<summary><b>macOS</b> · Apple Silicon</summary>
<br>

Download `tubify-0.1.1-macos-arm64.dmg` and drag Tubify to Applications.

It is ad-hoc signed, not notarized, so macOS will block the first launch. Open
**System Settings → Privacy & Security** and choose **Open Anyway**, or run:

```sh
xattr -dr com.apple.quarantine /Applications/Tubify.app
```

</details>

<details>
<summary><b>Windows</b> · x64</summary>
<br>

Download `tubify-0.1.1-windows-x86_64.zip`, extract it, and run
`bin\swiftify.exe`.

The exe is not signed, so SmartScreen may show “Windows protected your PC”.
Choose **More info → Run anyway**.

Keep `yt-dlp.exe` and `ffmpeg.exe` next to the executable or on `PATH`.

</details>

## build

<details>
<summary><b>What you need</b></summary>
<br>

- Qt **6.10 or newer** (Core, Gui, Network, Concurrent, Qml, Quick,
  QuickControls2, ShaderTools)
- CMake 3.24+, a C++20 compiler, Rust 1.85+
- [QCoro](https://github.com/qcoro/qcoro) 0.10+ (fetched automatically if it is
  not installed)
- `yt-dlp` and `ffmpeg` on `PATH` for playback
- Linux: `libpulse` for the audio-device monitor and ALSA for the core
- macOS: Xcode command line tools and CoreAudio, which Qt and `cpal` use
- Windows: Visual Studio 2022 with the C++ workload; WASAPI through `cpal`

The Material Symbols and Google Sans Flex fonts are downloaded at configure
time. Turn that off with `-DSWIFTIFY_DOWNLOAD_ICON_FONT=OFF` and
`-DSWIFTIFY_DOWNLOAD_INTERFACE_FONT=OFF` if you would rather supply them
yourself.

</details>

<details>
<summary><b>Linux</b></summary>
<br>

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j"$(nproc)"
./build/swiftify
```

On Arch:

```sh
sudo pacman -S qt6-base qt6-declarative qt6-shadertools qt6-tools \
  alsa-lib libpulse cmake ninja rust yt-dlp ffmpeg
```

If Qt or QCoro live in a custom prefix:

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$HOME/.local"
```

</details>

<details>
<summary><b>Linux</b> · faster rebuilds</summary>
<br>

The first build is slow; the rest need not be. With Ninja, ccache and lld:

```sh
sudo pacman -S ccache
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_CXX_COMPILER_LAUNCHER=ccache -DCMAKE_C_COMPILER_LAUNCHER=ccache \
  -DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=lld
cmake --build build -j"$(nproc)"
```

For day-to-day work, the Rust core can skip thin LTO to build quicker:

```sh
CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
  cmake --build build -j"$(nproc)"
```

</details>

<details>
<summary><b>macOS</b></summary>
<br>

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j"$(sysctl -n hw.ncpu)"
open build/Tubify.app
```

If Qt is installed with Homebrew, add its prefix:

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$(brew --prefix qt)"
```

To make a distributable disk image:

```sh
cmake --build build --config Release
cpack --config build/CPackConfig.cmake -G DragNDrop
```

Signing and notarization are optional. See `packaging/macos/` for the bundle
metadata and entitlements.

</details>

<details>
<summary><b>Windows</b></summary>
<br>

Use a Developer PowerShell and a 64-bit MSVC generator:

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
```

The executable is `build\Release\swiftify.exe` for now. Package it with the Qt
runtime and the installer:

```powershell
cpack --config build/CPackConfig.cmake -G NSIS
```

`yt-dlp.exe` and `ffmpeg.exe` still need to be on `PATH`, or placed next to the
executable. The installer can bundle them.

</details>

## first run

Tubify needs a YouTube Music account. There are two ways to give it one.

<details open>
<summary><b>Read the login from your browser</b> · the easy one</summary>
<br>

Open YouTube Music in a browser where you are already signed in, then choose
that browser on Tubify's setup screen:

**Helium · Chromium · Chrome · Brave · Firefox**

Tubify reads the cookie from that browser's own profile with `yt-dlp`. Close
the browser first if its cookie database is locked. Nothing is uploaded; the
cookie is kept on this computer and used for YouTube Music requests.

</details>

<details>
<summary><b>Paste the cookie yourself</b></summary>
<br>

1. Open [music.youtube.com](https://music.youtube.com) and sign in.
2. Press `F12`, open **Network**, and reload the page.
3. Click a request to `music.youtube.com`.
4. Copy the whole **Cookie** request header.
5. Paste it into Tubify's setup screen and choose **Sign In**.

The login is kept in Tubify's app-data folder, readable only by your user.
Nothing is sent anywhere except to YouTube Music.

</details>

## last.fm

The stats page can read a public Last.fm profile with just a username. It uses
a shared app key, so there is no API key to create.

<details>
<summary><b>Send plays to Last.fm</b> · scrobbling</summary>
<br>

1. Open **Settings → Listening Stats**.
2. Choose **Last.fm**.
3. Click **Connect Last.fm**, approve Tubify in the browser, then click
   **I Approved**.
4. Turn on **Scrobble plays to Last.fm**.

Scrobbles are sent when a play is recorded (after 30 seconds or half the
track). Last.fm itself can take a minute or two to show them.

</details>

## discord

Rich presence is one toggle in Settings. It uses a shared Discord app, so you
do not need to create an application or paste an ID. The song, artist, progress
and album art are shared while something is playing.

<details>
<summary><b>Covers for local files</b></summary>
<br>

Discord only shows images from the internet, never a file on your disk. For a
local song, Tubify looks its cover up on Deezer by artist and title and shows
that. Only the artist and title are sent, never the file name or path. If
Deezer has no match, the track, artist and progress still show without a cover.

</details>

## if something breaks

<details>
<summary><b>The browser login does nothing</b></summary>
<br>

Close the browser completely, then try again. Chromium-based browsers lock
their cookie database while they run.

</details>

<details>
<summary><b>Playback says it cannot fetch the song</b></summary>
<br>

Check that `yt-dlp` and `ffmpeg` are the system binaries, not an old pipx copy.
`yt-dlp --version` should be recent enough to know YouTube's current player.

</details>

<details>
<summary><b>A song 403s once and then works</b></summary>
<br>

YouTube occasionally rotates a stream. Retrying usually fixes it.

</details>

<details>
<summary><b>Scrobbles are late</b></summary>
<br>

Tubify sends them as soon as a play is recorded; Last.fm's own ingest can lag
by a minute or two.

</details>

<details>
<summary><b>Discord artwork is missing</b></summary>
<br>

Discord does not always accept external artwork URLs, and a local song's cover
depends on Deezer finding a match for its artist and title. The track, artist
and progress should still show.

</details>

<details>
<summary><b>macOS says the app is damaged</b></summary>
<br>

It is the quarantine flag, not a broken build. Use the `xattr` command in the
macOS install section above.

</details>

## notes and limits

<details>
<summary><b>Things Tubify cannot do</b></summary>
<br>

- YouTube Music does not let an app set a playlist's own cover, so that action
  reports that instead of pretending to work.
- Artist genres are not published by YouTube Music. The stats page shows the
  artist without a genre instead of guessing.
- Playback needs `yt-dlp` and `ffmpeg`. Without them the app still browses the
  library but cannot fetch a song.
- YouTube Music tops out at 256 kbps. There is no lossless or "HD audio" tier.
  The quality setting is Normal (128), High (256), or Best available.
- The bitrate shown in the player is the downloaded file's average, not a live
  reading.
- Closing the window quits by default. Turn on **Keep running in background**
  in Settings if you want it to hide and keep playing.

</details>

## credits

Tubify is a fork of [Swiftify](https://github.com/swiftify-org/swiftify) by
shred0s, which is GPL-3.0. The Material 3 tokens, motion and shapes come from
end4. The Rust core keeps Swiftify's audio pipeline, equalizer, analyser and
local-file playback.

Last.fm's shared read key comes from MusicBrainz Picard; the shared scrobbling
key comes from Last.fm Desktop. Both are GPL projects. Deezer provides the
artwork fallback.

## license

GPL-3.0. See [LICENSE](LICENSE).
