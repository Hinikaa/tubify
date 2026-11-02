//! what YouTube Music gives, shaped the way the front end already reads it.
//!
//! the qt side was written against spotify's web api. rather than teach every
//! view a second schema, the core hands back the same shapes: `images`,
//! `artists`, `tracks.total`, paged `items`, and so on. the ids inside are
//! youtube music's, and the uris are `ytm:` uris.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const TRACK_SCHEME: &str = "ytm:track:";
pub const ALBUM_SCHEME: &str = "ytm:album:";
pub const ARTIST_SCHEME: &str = "ytm:artist:";
pub const PLAYLIST_SCHEME: &str = "ytm:playlist:";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thumbnail {
    pub url: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

impl Thumbnail {
    pub fn new(url: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            url: url.into(),
            width,
            height,
        }
    }
}

/// the largest thumbnail at least `width` wide, or the largest there is.
pub fn pick_thumbnail(thumbnails: &[Thumbnail], width: u32) -> Option<&Thumbnail> {
    thumbnails
        .iter()
        .filter(|thumbnail| thumbnail.width >= width)
        .min_by_key(|thumbnail| thumbnail.width)
        .or_else(|| thumbnails.iter().max_by_key(|thumbnail| thumbnail.width))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
}

impl Artist {
    pub fn uri(&self) -> String {
        format!("{ARTIST_SCHEME}{}", self.id)
    }

    pub fn to_json(&self, width: u32) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "uri": self.uri(),
            "images": images_json(&self.thumbnails, width),
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Album {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
    #[serde(default)]
    pub release_date: String,
    #[serde(default)]
    pub track_count: u32,
    /// the album's audio playlist, when youtube music gave one: it lists the
    /// tracks even where the album page does not
    #[serde(default)]
    pub audio_playlist_id: String,
}

impl Album {
    pub fn uri(&self) -> String {
        format!("{ALBUM_SCHEME}{}", self.id)
    }

    pub fn artist_line(&self) -> String {
        self.artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn to_json(&self, width: u32) -> Value {
        json!({
            "id": self.id,
            "uri": self.uri(),
            "name": self.name,
            "artists": self.artists.iter().map(|artist| artist.to_json(width)).collect::<Vec<_>>(),
            "images": images_json(&self.thumbnails, width),
            "release_date": self.release_date,
            "total_tracks": self.track_count,
            "album_type": "album",
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub album: Option<Album>,
    #[serde(default)]
    pub duration_ms: u32,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
    /// the id of this occurrence inside a playlist, which removing and moving
    /// a row needs
    #[serde(default)]
    pub set_video_id: String,
    #[serde(default)]
    pub track_number: u32,
}

impl Track {
    pub fn uri(&self) -> String {
        format!("{TRACK_SCHEME}{}", self.id)
    }

    pub fn artist_line(&self) -> String {
        self.artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn album_name(&self) -> String {
        self.album
            .as_ref()
            .map(|album| album.name.clone())
            .unwrap_or_default()
    }

    pub fn artwork(&self) -> &[Thumbnail] {
        if !self.thumbnails.is_empty() {
            &self.thumbnails
        } else if let Some(album) = &self.album {
            &album.thumbnails
        } else {
            &[]
        }
    }

    pub fn to_json(&self, width: u32) -> Value {
        let album = self.album.as_ref().map(|album| album.to_json(width)).unwrap_or_else(|| json!({}));
        json!({
            "type": "track",
            "id": self.id,
            "uri": self.uri(),
            "name": self.title,
            "duration_ms": self.duration_ms,
            "explicit": self.explicit,
            "is_playable": true,
            "track_number": self.track_number,
            "images": images_json(self.artwork(), width),
            "artists": self.artists.iter().map(|artist| artist.to_json(width)).collect::<Vec<_>>(),
            "album": album,
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Owner {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
    #[serde(default)]
    pub owner: Option<Owner>,
    #[serde(default)]
    pub track_count: u32,
    #[serde(default = "default_true")]
    pub public: bool,
    #[serde(default)]
    pub editable: bool,
    /// `LM` is the liked music playlist; `SE` saved episodes
    #[serde(default)]
    pub system: String,
}

fn default_true() -> bool {
    true
}

impl Playlist {
    pub fn uri(&self) -> String {
        format!("{PLAYLIST_SCHEME}{}", self.id)
    }

    pub fn to_json(&self, width: u32) -> Value {
        let owner = self.owner.clone().unwrap_or_else(|| Owner {
            id: "youtube-music".to_owned(),
            name: "YouTube Music".to_owned(),
        });
        json!({
            "id": self.id,
            "uri": self.uri(),
            "name": self.name,
            "description": self.description,
            "images": images_json(&self.thumbnails, width),
            "owner": {
                "id": owner.id,
                "display_name": owner.name,
            },
            "public": self.public,
            "collaborative": false,
            "tracks": { "total": self.track_count },
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub handle: String,
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
}

impl Profile {
    pub fn to_json(&self, width: u32) -> Value {
        json!({
            "id": self.id,
            "display_name": self.name,
            "images": images_json(&self.thumbnails, width),
        })
    }
}

pub fn images_json(thumbnails: &[Thumbnail], width: u32) -> Value {
    let mut chosen: Vec<&Thumbnail> = thumbnails.iter().collect();
    chosen.sort_by_key(|thumbnail| thumbnail.width);
    let preferred = pick_thumbnail(thumbnails, width);
    let mut out = Vec::new();
    if let Some(preferred) = preferred {
        out.push(json!({
            "url": preferred.url,
            "width": preferred.width.max(width),
            "height": preferred.height.max(width),
        }));
    }
    for thumbnail in chosen {
        if preferred.is_some_and(|preferred| std::ptr::eq(preferred, thumbnail)) {
            continue;
        }
        out.push(json!({
            "url": thumbnail.url,
            "width": thumbnail.width,
            "height": thumbnail.height,
        }));
    }
    Value::Array(out)
}

/// a spotify-shaped page of songs.
pub fn track_page(tracks: &[Track], total: usize) -> Value {
    json!({
        "items": tracks.iter().map(|track| json!({ "item": track.to_json(640) })).collect::<Vec<_>>(),
        "total": total,
        "next": Value::Null,
    })
}

pub fn album_page(albums: &[Album]) -> Value {
    json!({
        "items": albums.iter().map(|album| album.to_json(640)).collect::<Vec<_>>(),
        "total": albums.len(),
        "next": Value::Null,
    })
}

pub fn artist_page(artists: &[Artist]) -> Value {
    json!({
        "items": artists.iter().map(|artist| artist.to_json(640)).collect::<Vec<_>>(),
        "total": artists.len(),
        "next": Value::Null,
    })
}

pub fn playlist_page(playlists: &[Playlist]) -> Value {
    json!({
        "items": playlists.iter().map(|playlist| playlist.to_json(640)).collect::<Vec<_>>(),
        "total": playlists.len(),
        "next": Value::Null,
    })
}

/// the id at the end of a `ytm:` uri, or the input when it is bare.
pub fn id_from_uri(uri: &str) -> String {
    uri.rsplit(':').next().unwrap_or(uri).to_owned()
}

/// a `VL`-prefixed browse id for a playlist, as youtube music's browse
/// endpoint wants it.
pub fn playlist_browse_id(id: &str) -> String {
    let id = id.strip_prefix("VL").unwrap_or(id);
    format!("VL{id}")
}

pub fn is_album_browse_id(id: &str) -> bool {
    id.starts_with("MPRE")
}

pub fn is_artist_browse_id(id: &str) -> bool {
    id.starts_with("UC")
}

pub fn is_playlist_browse_id(id: &str) -> bool {
    id.starts_with("VL") || id.starts_with("PL") || id == "LM" || id.starts_with("RD")
}
