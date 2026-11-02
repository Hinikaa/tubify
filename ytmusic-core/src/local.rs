/*
 * Swiftify, a spotify player for desktop
 * Copyright (C) 2026  shred0s
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

//! music files of the person's own, outside spotify: what a file's tags say
//! about it, its cover, and how long it runs. it plays straight from disk
//! (SpotifyCore::play_file_as).

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use lofty::{
    config::WriteOptions,
    file::{AudioFile, FileType, TaggedFileExt},
    picture::{MimeType, Picture, PictureType},
    probe::Probe,
    tag::{Accessor, ItemKey, Tag, TagType},
};
use serde::{Deserialize, Serialize};
use symphonia::core::{
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
    meta::{MetadataOptions, MetadataRevision, StandardTagKey, StandardVisualKey, Value, Visual},
    probe::Hint,
    units::TimeBase,
};

/// the file extensions the decoder plays
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "m4a", "mp4", "aac", "alac", "wav", "aif", "aiff",
];

/// the names a cover kept beside the files of an album goes by
const FOLDER_COVERS: &[&str] = &["cover", "folder", "front", "album", "albumart", "artwork"];
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// what a file says about itself. anything it does not say is left out, for
/// the front end to fill in from the file's name and folder.
#[derive(Serialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub duration_ms: u64,
    /// the cover: one embedded in the file, written out into the artwork
    /// folder under a name taken from its contents, or one kept beside it
    pub artwork_path: Option<String>,
}

/// what an edit changes in a file's tags: each field given is set, one given
/// empty is cleared, and one left out stays as it is
#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct TagEdits {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
    /// as typed: a number, or empty to clear it
    pub track_number: Option<String>,
    pub disc_number: Option<String>,
    /// takes every picture out; a new cover given as well goes in after
    pub remove_artwork: bool,
}

/// writes `edits` into the file at `path`, with `artwork` (an image's bytes)
/// as its front cover when given. it is written into a copy beside the file,
/// which takes the file's place only once it is whole: a write that fails
/// leaves the song as it was
pub fn write_tags(path: &Path, edits: &TagEdits, artwork: Option<&[u8]>) -> Result<(), String> {
    let mut file = Probe::open(path)
        .and_then(|probe| probe.read())
        .map_err(|error| format!("could not read the tags of {}: {error}", path.display()))?;
    let file_type = file.file_type();

    // every tag the file keeps that can be written, its usual one, and for
    // wav the list the player itself reads: all say the same afterwards
    let mut kinds: Vec<TagType> = file.tags().iter().map(Tag::tag_type).collect();
    kinds.push(file.primary_tag_type());
    if file_type == FileType::Wav {
        kinds.push(TagType::RiffInfo);
    }
    kinds.retain(|kind| file_type.tag_support(*kind).is_writable());
    kinds.dedup();
    let mut seen = Vec::new();
    kinds.retain(|kind| {
        let new = !seen.contains(kind);
        seen.push(*kind);
        new
    });
    if kinds.is_empty() {
        return Err(format!("{} cannot hold tags", path.display()));
    }

    for kind in kinds {
        let mut tag = file.remove(kind).unwrap_or_else(|| Tag::new(kind));
        apply_edits(&mut tag, edits, artwork);
        file.insert_tag(tag);
    }

    let directory = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let partial = directory.join(format!(".{name}.{}.swiftify-tags", std::process::id()));
    let written = fs::copy(path, &partial)
        .map_err(|error| format!("could not copy {}: {error}", path.display()))
        .and_then(|_| {
            file.save_to_path(&partial, WriteOptions::default())
                .map_err(|error| format!("could not write the tags of {}: {error}", path.display()))
        })
        .and_then(|()| {
            fs::rename(&partial, path)
                .map_err(|error| format!("could not replace {}: {error}", path.display()))
        });
    if written.is_err() {
        let _ = fs::remove_file(&partial);
    }
    written
}

fn apply_edits(tag: &mut Tag, edits: &TagEdits, artwork: Option<&[u8]>) {
    fn text(value: &Option<String>) -> Option<Option<String>> {
        value.as_ref().map(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        })
    }
    if let Some(value) = text(&edits.title) {
        match value {
            Some(value) => tag.set_title(value),
            None => tag.remove_title(),
        }
    }
    if let Some(value) = text(&edits.artist) {
        match value {
            Some(value) => tag.set_artist(value),
            None => tag.remove_artist(),
        }
    }
    if let Some(value) = text(&edits.album) {
        match value {
            Some(value) => tag.set_album(value),
            None => tag.remove_album(),
        }
    }
    if let Some(value) = text(&edits.genre) {
        match value {
            Some(value) => tag.set_genre(value),
            None => tag.remove_genre(),
        }
    }
    for (key, value) in [
        (ItemKey::AlbumArtist, &edits.album_artist),
        (ItemKey::RecordingDate, &edits.date),
    ] {
        if let Some(value) = text(value) {
            tag.remove_key(key.clone());
            if let Some(value) = value {
                tag.insert_text(key, value);
            }
        }
    }
    if let Some(value) = text(&edits.track_number) {
        match value.as_deref().and_then(leading_number) {
            Some(number) => tag.set_track(number),
            None => tag.remove_track(),
        }
    }
    if let Some(value) = text(&edits.disc_number) {
        match value.as_deref().and_then(leading_number) {
            Some(number) => tag.set_disk(number),
            None => tag.remove_disk(),
        }
    }
    if edits.remove_artwork {
        while tag.picture_count() > 0 {
            tag.remove_picture(0);
        }
    }
    if let Some(bytes) = artwork.filter(|bytes| !bytes.is_empty()) {
        tag.remove_picture_type(PictureType::CoverFront);
        let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            MimeType::Png
        } else {
            MimeType::Jpeg
        };
        tag.push_picture(
            Picture::unchecked(bytes.to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build(),
        );
    }
}

/// the front cover lofty finds when symphonia found none: some files keep
/// theirs where symphonia does not look (a wav's id3 chunk)
fn lofty_cover(path: &Path) -> Option<Visual> {
    let file = Probe::open(path).ok()?.read().ok()?;
    let picture = file.tags().iter().find_map(|tag| {
        tag.get_picture_type(PictureType::CoverFront)
            .or_else(|| tag.pictures().first())
    })?;
    Some(Visual {
        media_type: picture
            .mime_type()
            .map(|mime| mime.as_str().to_owned())
            .unwrap_or_else(|| "image/jpeg".to_owned()),
        dimensions: None,
        bits_per_pixel: None,
        color_mode: None,
        usage: Some(StandardVisualKey::FrontCover),
        tags: Vec::new(),
        data: picture.data().to_vec().into_boxed_slice(),
    })
}

fn open(path: &Path) -> Result<symphonia::core::probe::ProbeResult, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| format!("could not read {}: {error}", path.display()))
}

/// the tags of the file at `path`, with an embedded cover written into
/// `artwork_dir` when there is one
pub fn read_tags(path: &Path, artwork_dir: Option<&Path>) -> Result<LocalTags, String> {
    let mut probed = open(path)?;
    let mut tags = LocalTags::default();

    // the container's own tags first (vorbis comments, mp4 atoms), then
    // any found ahead of it (an mp3's id3v2): whichever says a thing first
    let mut cover: Option<Visual> = None;
    if let Some(revision) = probed.format.metadata().skip_to_latest() {
        apply(&mut tags, revision, &mut cover);
    }
    if let Some(mut metadata) = probed.metadata.get()
        && let Some(revision) = metadata.skip_to_latest()
    {
        apply(&mut tags, revision, &mut cover);
    }

    tags.duration_ms = duration_ms(probed.format.as_mut());
    if cover.is_none() && artwork_dir.is_some() {
        cover = lofty_cover(path);
    }
    tags.artwork_path = match (cover, artwork_dir) {
        (Some(visual), Some(directory)) => write_cover(&visual, directory).ok(),
        _ => None,
    }
    .or_else(|| folder_cover(path))
    .map(|cover| cover.to_string_lossy().into_owned());
    Ok(tags)
}

/// how long the file at `path` runs, for a decoder that cannot say
pub fn measure_duration_ms(path: &Path) -> Option<u64> {
    let mut probed = open(path).ok()?;
    Some(duration_ms(probed.format.as_mut())).filter(|duration| *duration > 0)
}

/// the linear gain a file's replaygain tags ask for, capped at its peak so
/// nothing clips, or 1.0 when the file says nothing
pub fn replay_gain(path: &Path) -> f32 {
    let Ok(file) = Probe::open(path).and_then(|probe| probe.read()) else {
        return 1.0;
    };
    let find = |key: ItemKey| {
        file.tags()
            .iter()
            .find_map(|tag| tag.get_string(key.clone()))
            .and_then(parse_replay_gain_value)
    };
    let Some(gain_db) = find(ItemKey::ReplayGainTrackGain)
        .or_else(|| find(ItemKey::ReplayGainAlbumGain))
    else {
        return 1.0;
    };
    let peak = find(ItemKey::ReplayGainTrackPeak)
        .or_else(|| find(ItemKey::ReplayGainAlbumPeak))
        .filter(|peak| *peak > 0.0)
        .unwrap_or(1.0);
    (10f32.powf(gain_db / 20.0)).min(1.0 / peak).clamp(0.0, 4.0)
}

fn parse_replay_gain_value(text: &str) -> Option<f32> {
    let trimmed = text
        .trim()
        .trim_end_matches(|character: char| character.is_ascii_alphabetic() || character.is_whitespace());
    trimmed.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn apply(tags: &mut LocalTags, revision: &MetadataRevision, cover: &mut Option<Visual>) {
    for tag in revision.tags() {
        let text = text_of(&tag.value);
        if text.is_empty() {
            continue;
        }
        let Some(key) = tag.std_key else {
            // a few writers name the track number in ways symphonia does not
            // know: wav's "ITRK", and plain "TRACK" comments
            if matches!(
                tag.key.to_ascii_lowercase().as_str(),
                "itrk" | "track" | "tracknumber"
            ) {
                tags.track_number = tags.track_number.or_else(|| leading_number(&text));
            }
            continue;
        };
        let field = match key {
            StandardTagKey::TrackTitle => &mut tags.title,
            StandardTagKey::Artist => &mut tags.artist,
            StandardTagKey::Album => &mut tags.album,
            StandardTagKey::AlbumArtist => &mut tags.album_artist,
            StandardTagKey::Genre => &mut tags.genre,
            StandardTagKey::Date | StandardTagKey::ReleaseDate | StandardTagKey::OriginalDate => {
                &mut tags.date
            }
            StandardTagKey::TrackNumber => {
                tags.track_number = tags.track_number.or_else(|| leading_number(&text));
                continue;
            }
            StandardTagKey::DiscNumber => {
                tags.disc_number = tags.disc_number.or_else(|| leading_number(&text));
                continue;
            }
            _ => continue,
        };
        if field.is_none() {
            *field = Some(text);
        }
    }
    if cover.is_none() {
        // the front cover, or failing that whatever picture there is
        let visuals = revision.visuals();
        *cover = visuals
            .iter()
            .find(|visual| visual.usage == Some(StandardVisualKey::FrontCover))
            .or_else(|| visuals.first())
            .filter(|visual| !visual.data.is_empty())
            .cloned();
    }
}

fn text_of(value: &Value) -> String {
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Binary(_) => String::new(),
        other => other.to_string(),
    };
    // id3 strings may carry their terminator
    text.trim_matches(|character: char| character == '\0' || character.is_whitespace())
        .to_owned()
}

/// "3", "03" and "3/12" are all track three
fn leading_number(text: &str) -> Option<u32> {
    let digits: String = text
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok().filter(|number| *number > 0)
}

fn duration_ms(format: &mut dyn FormatReader) -> u64 {
    let Some(track) = format.default_track() else {
        return 0;
    };
    let id = track.id;
    let params = track.codec_params.clone();
    if let (Some(frames), Some(rate)) = (params.n_frames, params.sample_rate)
        && rate > 0
    {
        return frames * 1000 / u64::from(rate);
    }
    // nothing in the header says (a variable rate mp3 without one): count
    // the packets, which reads the file without decoding it
    let Some(time_base) = params
        .time_base
        .or_else(|| params.sample_rate.map(|rate| TimeBase::new(1, rate)))
    else {
        return 0;
    };
    let mut total = 0u64;
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() == id {
            total += packet.dur;
        }
    }
    let time = time_base.calc_time(total);
    time.seconds * 1000 + (time.frac * 1000.0) as u64
}

fn write_cover(visual: &Visual, directory: &Path) -> io::Result<PathBuf> {
    let extension = match visual.media_type.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => "jpg",
    };
    // named by what it holds: an album's tracks all share one file
    let mut hasher = DefaultHasher::new();
    visual.data.hash(&mut hasher);
    let path = directory.join(format!("{:016x}.{extension}", hasher.finish()));
    if !path.exists() {
        fs::create_dir_all(directory)?;
        // written aside and moved into place, so a reader never sees half,
        // under a name of its own should two files of an album race
        static WRITES: AtomicU64 = AtomicU64::new(0);
        let partial = directory.join(format!(
            "{:016x}.{}.{}.partial",
            hasher.finish(),
            std::process::id(),
            WRITES.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&partial, &visual.data)?;
        fs::rename(&partial, &path)?;
    }
    Ok(path)
}

/// a cover kept as a picture in the album's folder
fn folder_cover(path: &Path) -> Option<PathBuf> {
    let folder = path.parent()?;
    let entries = fs::read_dir(folder).ok()?;
    let mut found: Option<(usize, PathBuf)> = None;
    for entry in entries.flatten() {
        let candidate = entry.path();
        let (Some(stem), Some(extension)) = (
            candidate.file_stem().and_then(|stem| stem.to_str()),
            candidate
                .extension()
                .and_then(|extension| extension.to_str()),
        ) else {
            continue;
        };
        if !IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) {
            continue;
        }
        let stem = stem.to_ascii_lowercase();
        // in the order of the list: a "cover" beats a "folder"
        if let Some(rank) = FOLDER_COVERS.iter().position(|name| stem == *name)
            && found.as_ref().is_none_or(|(best, _)| rank < *best)
        {
            found = Some((rank, candidate));
        }
    }
    found.map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a second of silence as a 16-bit wav, with a LIST INFO chunk of tags
    fn tagged_wav(path: &Path) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..44_100 * 2 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();

        // hound writes no tags; a LIST INFO chunk goes in ahead of the data,
        // where readers look for it
        let info = |id: &[u8; 4], text: &str| {
            let mut bytes = id.to_vec();
            let mut value = text.as_bytes().to_vec();
            value.push(0);
            if value.len() % 2 == 1 {
                value.push(0);
            }
            bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
            bytes.extend(value);
            bytes
        };
        let mut list = b"INFO".to_vec();
        list.extend(info(b"INAM", "Smooth Operator"));
        list.extend(info(b"IART", "Sade"));
        list.extend(info(b"IPRD", "Diamond Life"));
        list.extend(info(b"ITRK", "4/9"));
        list.extend(info(b"IGNR", "Soul"));
        let mut chunk = b"LIST".to_vec();
        chunk.extend_from_slice(&(list.len() as u32).to_le_bytes());
        chunk.extend(list);

        let mut bytes = fs::read(path).unwrap();
        let data = bytes
            .windows(4)
            .position(|window| window == b"data")
            .unwrap();
        bytes.splice(data..data, chunk);
        let riff_size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_tagged_file_says_what_it_is_and_how_long_it_runs() {
        let directory = std::env::temp_dir().join(format!("swiftify-local-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("04 smooth.wav");
        tagged_wav(&path);

        let tags = read_tags(&path, None).unwrap();
        assert_eq!(tags.title.as_deref(), Some("Smooth Operator"));
        assert_eq!(tags.artist.as_deref(), Some("Sade"));
        assert_eq!(tags.album.as_deref(), Some("Diamond Life"));
        assert_eq!(tags.track_number, Some(4));
        assert_eq!(tags.genre.as_deref(), Some("Soul"));
        assert!(
            (990..=1010).contains(&tags.duration_ms),
            "{}",
            tags.duration_ms
        );
        assert_eq!(tags.artwork_path, None);

        // a cover beside it is found, and preferred by name
        fs::write(directory.join("folder.png"), b"png").unwrap();
        fs::write(directory.join("Cover.jpg"), b"jpg").unwrap();
        let tags = read_tags(&path, None).unwrap();
        assert!(tags.artwork_path.unwrap().ends_with("Cover.jpg"));

        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn edits_are_written_into_the_file_and_read_back() {
        let directory = std::env::temp_dir().join(format!("swiftify-edit-{}", std::process::id()));
        let artwork = directory.join("artwork");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("04 smooth.wav");
        tagged_wav(&path);
        let cover = [0xff, 0xd8, 0xff, 0xe0, 1, 2, 3, 4, 5, 6, 7, 8];

        let edits = TagEdits {
            title: Some("Your Love Is King".to_owned()),
            artist: Some("  Sade  ".to_owned()),
            album_artist: Some("Sade".to_owned()),
            track_number: Some("2".to_owned()),
            disc_number: Some("1".to_owned()),
            // left out: the album stays as it was
            ..TagEdits::default()
        };
        write_tags(&path, &edits, Some(&cover)).unwrap();
        let tags = read_tags(&path, Some(&artwork)).unwrap();
        assert_eq!(tags.title.as_deref(), Some("Your Love Is King"));
        assert_eq!(tags.artist.as_deref(), Some("Sade"));
        assert_eq!(tags.album.as_deref(), Some("Diamond Life"));
        assert_eq!(tags.track_number, Some(2));
        assert!(
            (990..=1010).contains(&tags.duration_ms),
            "{}",
            tags.duration_ms
        );
        let kept = tags.artwork_path.expect("the cover reads back");
        assert_eq!(fs::read(kept).unwrap(), cover);
        // and nothing is left beside it
        let leftovers: Vec<_> = fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".swiftify-tags")
            })
            .collect();
        assert!(leftovers.is_empty());

        // emptied, a field goes; the cover can go too
        let edits = TagEdits {
            genre: Some(String::new()),
            remove_artwork: true,
            ..TagEdits::default()
        };
        write_tags(&path, &edits, None).unwrap();
        let tags = read_tags(&path, Some(&artwork)).unwrap();
        assert_eq!(tags.genre, None);
        assert_eq!(tags.title.as_deref(), Some("Your Love Is King"));
        assert_eq!(tags.artwork_path, None);

        // a file that is not music is refused, and left alone
        let other = directory.join("notes.mp3");
        fs::write(&other, b"not music").unwrap();
        assert!(write_tags(&other, &TagEdits::default(), None).is_err());
        assert_eq!(fs::read(&other).unwrap(), b"not music");

        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn track_numbers_come_in_several_spellings() {
        assert_eq!(leading_number("3"), Some(3));
        assert_eq!(leading_number("03/12"), Some(3));
        assert_eq!(leading_number(" 7 "), Some(7));
        assert_eq!(leading_number("side b"), None);
        assert_eq!(leading_number("0"), None);
    }

    #[test]
    fn a_file_that_is_not_music_is_refused() {
        let path =
            std::env::temp_dir().join(format!("swiftify-not-music-{}.mp3", std::process::id()));
        fs::write(&path, b"this is not an mp3").unwrap();
        assert!(read_tags(&path, None).is_err());
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn replay_gain_values_are_parsed_with_or_without_db_suffix() {
        assert_eq!(parse_replay_gain_value("-6.5 dB"), Some(-6.5));
        assert_eq!(parse_replay_gain_value("+2.10 dB"), Some(2.1));
        assert_eq!(parse_replay_gain_value("0.988553"), Some(0.988553));
        assert_eq!(parse_replay_gain_value("not a gain"), None);
    }
}
