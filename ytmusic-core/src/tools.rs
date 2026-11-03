/*
 * Tubify, a YouTube Music player for desktop
 * Copyright (C) 2026  Hinika
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! the outside programs the core runs: yt-dlp fetches a song, ffmpeg brings an
//! odd container to m4a.
//!
//! both are looked for beside the app first, so the copy shipped with it is the
//! one that runs, then in the per-user place an installer fills, and only then
//! on the PATH. on linux the distro's own binary is preferred over whatever a
//! pipx or mise shim puts first on the PATH, since an older shim can fail on a
//! video the distro's newer one plays.

use std::path::PathBuf;

use tokio::process::Command;

#[cfg(windows)]
const EXTENSION: &str = ".exe";
#[cfg(not(windows))]
const EXTENSION: &str = "";

/// started without a console window of its own: a windowed app on windows would
/// otherwise flash a black box for every song it fetches
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn beside_app(file: &str) -> Vec<PathBuf> {
    let Ok(executable) = std::env::current_exe() else {
        return Vec::new();
    };
    let Some(directory) = executable.parent() else {
        return Vec::new();
    };
    // the executable's own folder, and the two a package puts its tools in
    vec![
        directory.join(file),
        directory.join("bin").join(file),
        directory.join("tools").join(file),
    ]
}

#[cfg(windows)]
fn user_directory() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("Tubify").join("bin"))
}

#[cfg(not(windows))]
fn user_directory() -> Option<PathBuf> {
    if let Some(base) = std::env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(base).join("tubify").join("bin"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local").join("bin"))
}

/// every place a tool is looked for, in the order they are tried.
fn candidates(name: &str) -> Vec<PathBuf> {
    let file = format!("{name}{EXTENSION}");
    let mut paths = beside_app(&file);
    if let Some(directory) = user_directory() {
        paths.push(directory.join(&file));
    }
    #[cfg(not(windows))]
    {
        paths.push(PathBuf::from("/usr/bin").join(&file));
        paths.push(PathBuf::from("/usr/local/bin").join(&file));
    }
    paths
}

fn locate(name: &str) -> Option<PathBuf> {
    candidates(name).into_iter().find(|path| path.is_file())
}

fn tool(name: &str) -> Command {
    let command = match locate(name) {
        Some(path) => Command::new(path),
        // nothing of ours: the PATH decides, which is all a distro package
        // needs
        None => Command::new(name),
    };
    #[cfg(windows)]
    let mut command = command;
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub fn yt_dlp() -> Command {
    tool("yt-dlp")
}

pub fn ffmpeg() -> Command {
    tool("ffmpeg")
}

/// what to say when there is no yt-dlp anywhere: the way to get one on this
/// system.
pub fn missing_yt_dlp() -> &'static str {
    #[cfg(windows)]
    {
        "yt-dlp is not installed. Install it with \"winget install yt-dlp.yt-dlp\", \
         or put yt-dlp.exe beside Tubify, so Tubify can fetch songs."
    }
    #[cfg(not(windows))]
    {
        "yt-dlp is not installed. Install it with your package manager \
         (pacman -S yt-dlp, or apt install yt-dlp) so Tubify can fetch songs."
    }
}

/// the same, for the converter, which only an odd container needs.
pub fn missing_ffmpeg() -> &'static str {
    #[cfg(windows)]
    {
        "ffmpeg is not installed. Install it with \"winget install Gyan.FFmpeg\", \
         or put ffmpeg.exe beside Tubify."
    }
    #[cfg(not(windows))]
    {
        "ffmpeg is not installed. Install it with your package manager \
         (pacman -S ffmpeg, or apt install ffmpeg)."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_folder_is_searched_before_anything_else() {
        let executable = std::env::current_exe().unwrap();
        let beside = executable.parent().unwrap().join(format!("yt-dlp{EXTENSION}"));
        assert_eq!(candidates("yt-dlp").first(), Some(&beside));
    }

    #[test]
    fn the_user_directory_is_searched_before_the_path() {
        let Some(user) = user_directory() else {
            return;
        };
        let paths = candidates("ffmpeg");
        let position = paths.iter().position(|path| *path == user.join(format!("ffmpeg{EXTENSION}")));
        assert_eq!(position, Some(beside_app(&format!("ffmpeg{EXTENSION}")).len()));
    }

    #[test]
    fn a_tool_nowhere_is_left_to_the_path() {
        assert!(locate("a-tool-that-is-not-installed-anywhere").is_none());
    }

    #[test]
    fn the_missing_message_names_a_way_to_install_it() {
        assert!(missing_yt_dlp().contains("yt-dlp"));
        assert!(missing_ffmpeg().contains("ffmpeg"));
    }
}
