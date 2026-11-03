//! a song's audio, fetched with yt-dlp.
//!
//! youtube music hands out ciphered stream urls that have to be deciphered
//! before they play. yt-dlp already does that, follows the format changes, and
//! never puts an ad in the file, so it does the fetching and the core only
//! decodes what it wrote.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use crate::tools::{ffmpeg, missing_yt_dlp, yt_dlp};

const MIN_CACHED_BYTES: u64 = 100_000;

/// the format selector for a stream quality, in kbps. zero is "best".
fn format(quality_kbps: u32) -> String {
    match quality_kbps {
        0 => "bestaudio[ext=m4a]/bestaudio[ext=mp4]/bestaudio".to_owned(),
        1..=128 => "bestaudio[abr<=128][ext=m4a]/bestaudio[abr<=128]/bestaudio".to_owned(),
        _ => "bestaudio[abr<=256][ext=m4a]/bestaudio[abr<=256]/bestaudio".to_owned(),
    }
}

pub fn cached_file(directory: &Path, video_id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(video_id) || name.ends_with(".part") || name.ends_with(".ytdl") {
            continue;
        }
        let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        if size > MIN_CACHED_BYTES {
            return Some(path);
        }
    }
    None
}

/// downloads `video_id` into `directory` (unless it is already there) and
/// returns the file.
///
/// an anonymous request is tried first: it is the one that always works and
/// the one a filtered resolver would make. if youtube refuses it, the account's
/// cookie is offered as a second attempt, which is what gets past a sign-in
/// wall or a premium-only song.
pub async fn download(
    video_id: &str,
    directory: &Path,
    quality_kbps: u32,
    cookie: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(path) = cached_file(directory, video_id) {
        return Ok(path);
    }
    std::fs::create_dir_all(directory).map_err(|error| {
        format!(
            "could not create the audio cache at {}: {error}",
            directory.display()
        )
    })?;

    let first = run(video_id, directory, quality_kbps, None).await;
    match first {
        Ok(path) => Ok(path),
        Err(anonymous) => {
            let Some(cookie) = cookie else {
                return Err(anonymous);
            };
            let Some(cookie_file) = write_cookies(directory, cookie) else {
                return Err(anonymous);
            };
            match run(video_id, directory, quality_kbps, Some(&cookie_file)).await {
                Ok(path) => Ok(path),
                Err(_) => Err(anonymous),
            }
        }
    }
}

async fn run(
    video_id: &str,
    directory: &Path,
    quality_kbps: u32,
    cookie_file: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(path) = cached_file(directory, video_id) {
        return Ok(path);
    }
    let output_template = directory.join(format!("{video_id}.%(ext)s"));
    let watch_url = format!("https://music.youtube.com/watch?v={video_id}");

    let mut command = yt_dlp();
    if let Some(cookie_file) = cookie_file {
        command.arg("--cookies").arg(cookie_file);
    }
    command
        .arg("-f")
        .arg(format(quality_kbps))
        .arg("--no-playlist")
        .arg("--no-warnings")
        .arg("--no-progress")
        .arg("--socket-timeout")
        .arg("20")
        .arg("--retries")
        .arg("3")
        .arg("--fragment-retries")
        .arg("3")
        .arg("-o")
        .arg(&output_template)
        .arg(&watch_url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let output = tokio::time::timeout(Duration::from_secs(180), command.output())
        .await
        .map_err(|_| "Fetching the song from YouTube Music timed out.".to_owned())?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                missing_yt_dlp().to_owned()
            } else {
                format!("could not start yt-dlp: {error}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let line = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("yt-dlp failed");
        return Err(format!(
            "YouTube Music would not give this song: {}",
            line.trim()
        ));
    }

    if let Some(path) = cached_file(directory, video_id) {
        return Ok(path);
    }

    // an odd container (webm/opus) is brought to m4a, which the decoder reads
    // natively. most songs come as m4a already.
    let candidates: Vec<PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(video_id) && !name.ends_with(".part"))
        })
        .collect();
    let Some(source) = candidates
        .into_iter()
        .max_by_key(|path| std::fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0))
    else {
        return Err("yt-dlp finished but wrote no audio.".to_owned());
    };
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        extension.as_str(),
        "m4a" | "mp4" | "mp3" | "flac" | "wav" | "ogg" | "oga" | "opus"
    ) {
        return Ok(source);
    }

    let target = directory.join(format!("{video_id}.m4a"));
    let status = ffmpeg()
        .arg("-y")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(&source)
        .arg("-vn")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("256k")
        .arg(&target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                crate::tools::missing_ffmpeg().to_owned()
            } else {
                format!("could not convert the song with ffmpeg: {error}")
            }
        })?;
    if !status.success() {
        return Err("ffmpeg could not convert the downloaded song.".to_owned());
    }
    let _ = std::fs::remove_file(&source);
    Ok(target)
}

/// yt-dlp reads cookies in netscape's format; the browser's Cookie header is
/// turned into that, in a file only its owner can read.
fn write_cookies(directory: &Path, cookie: &str) -> Option<PathBuf> {
    let path = directory.join(".yt-dlp-cookies.txt");
    let mut lines = String::from("# Netscape HTTP Cookie File\n");
    for part in cookie.split(';') {
        let part = part.trim();
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        lines.push_str(&format!(
            ".youtube.com\tTRUE\t/\tFALSE\t0\t{key}\t{value}\n"
        ));
    }
    std::fs::write(&path, lines).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Some(path)
}

/// deletes the oldest cached songs until the directory is under `limit_bytes`.
pub fn trim_cache(directory: &Path, limit_bytes: u64) {
    if limit_bytes == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            Some((
                metadata.modified().unwrap_or(std::time::UNIX_EPOCH),
                metadata.len(),
                path,
            ))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, size, _)| *size).sum();
    if total <= limit_bytes {
        return;
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, size, path) in files {
        if total <= limit_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_best_format_is_the_default() {
        assert!(format(0).starts_with("bestaudio"));
        assert!(format(320).starts_with("bestaudio"));
    }

    #[test]
    fn a_low_quality_caps_the_bitrate() {
        assert!(format(128).contains("abr<=128"));
    }
}
