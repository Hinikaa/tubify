//! reads innertube's renderer trees into the small model the app uses.
//!
//! youtube music lays the same data out differently from page to page (a
//! playlist header is not an album header, and a search shelf is not a
//! library grid), so the helpers here look for a renderer wherever it sits
//! rather than following one fixed path. that also keeps a layout change from
//! taking the whole page down.

use serde_json::Value;

use super::models::{
    Album, Artist, Owner, Playlist, Thumbnail, Track, is_album_browse_id, is_artist_browse_id,
};

const TYPE_WORDS: [&str; 12] = [
    "Song", "Video", "Album", "Single", "EP", "Playlist", "Artist", "Profile", "Channel",
    "Podcast", "Episode", "Upload",
];

// ---------------------------------------------------------------------------
// walking the tree
// ---------------------------------------------------------------------------

pub fn collect<'a>(value: &'a Value, key: &str) -> Vec<&'a Value> {
    fn walk<'a>(value: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
        match value {
            Value::Object(map) => {
                if let Some(found) = map.get(key) {
                    out.push(found);
                }
                for child in map.values() {
                    walk(child, key, out);
                }
            }
            Value::Array(items) => {
                for child in items {
                    walk(child, key, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(value, key, &mut out);
    out
}

pub fn first<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key) {
                return Some(found);
            }
            map.values().find_map(|child| first(child, key))
        }
        Value::Array(items) => items.iter().find_map(|child| first(child, key)),
        _ => None,
    }
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for step in path {
        current = current.get(*step)?;
    }
    current.as_str()
}

/// a text renderer's words: `runs`, `simpleText`, or a bare string.
pub fn text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_owned();
    }
    if let Some(text) = value.get("simpleText").and_then(Value::as_str) {
        return text.to_owned();
    }
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return text.to_owned();
    }
    if let Some(runs) = value.get("runs").and_then(Value::as_array) {
        return runs
            .iter()
            .filter_map(|run| run.get("text").and_then(Value::as_str))
            .collect();
    }
    String::new()
}

pub fn thumbnails(value: &Value) -> Vec<Thumbnail> {
    let arrays = collect(value, "thumbnails");
    for array in arrays {
        let Some(items) = array.as_array() else {
            continue;
        };
        let parsed: Vec<Thumbnail> = items
            .iter()
            .filter_map(|item| {
                let url = item.get("url")?.as_str()?;
                if url.is_empty() {
                    return None;
                }
                Some(Thumbnail::new(
                    url,
                    item.get("width").and_then(Value::as_u64).unwrap_or(0) as u32,
                    item.get("height").and_then(Value::as_u64).unwrap_or(0) as u32,
                ))
            })
            .collect();
        if !parsed.is_empty() {
            return parsed;
        }
    }
    Vec::new()
}

pub fn browse_id(value: &Value) -> Option<String> {
    // the card's own destination first: a menu below it has its own browse
    // endpoints ("go to artist", "remove from library") that must not win
    let direct = value
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .or_else(|| value.pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty());
    if let Some(id) = direct {
        return Some(id.to_owned());
    }
    collect(value, "browseEndpoint")
        .into_iter()
        .find_map(|endpoint| {
            endpoint
                .get("browseId")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
        })
}

pub fn video_id(value: &Value) -> Option<String> {
    if let Some(id) = string_at(value, &["playlistItemData", "videoId"]) {
        return Some(id.to_owned());
    }
    collect(value, "watchEndpoint").into_iter().find_map(|endpoint| {
        endpoint
            .get("videoId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
    })
}

pub fn set_video_id(value: &Value) -> String {
    string_at(value, &["playlistItemData", "playlistSetVideoId"])
        .or_else(|| string_at(value, &["playlistItemData", "setVideoId"]))
        .or_else(|| {
            collect(value, "playlistItemData")
                .into_iter()
                .find_map(|data| {
                    data.get("playlistSetVideoId")
                        .or_else(|| data.get("setVideoId"))
                        .and_then(Value::as_str)
                })
        })
        .unwrap_or_default()
        .to_owned()
}

fn page_type(value: &Value) -> Option<String> {
    collect(value, "browseEndpointContextMusicConfig")
        .into_iter()
        .find_map(|config| {
            config
                .get("pageType")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn run_text(run: &Value) -> String {
    run.get("text").and_then(Value::as_str).unwrap_or_default().to_owned()
}

fn run_browse_id(run: &Value) -> Option<String> {
    run.pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

fn run_page_type(run: &Value) -> Option<String> {
    run.pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn runs(value: &Value) -> Vec<&Value> {
    value
        .get("runs")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// small pieces
// ---------------------------------------------------------------------------

/// "3:42", "1:02:03" in seconds. youtube sometimes gives "3:42" as a run and
/// sometimes only in a fixed column.
pub fn clock_seconds(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || !text.contains(':') {
        return None;
    }
    let mut total = 0u32;
    let mut parts = 0u32;
    for field in text.split(':') {
        if field.is_empty() || field.len() > 3 || !field.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        total = total * 60 + field.parse::<u32>().ok()?;
        parts += 1;
    }
    (parts >= 2).then_some(total)
}

fn number_before(text: &str, words: &[&str]) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    for word in words {
        if let Some(index) = lower.find(word) {
            let prefix = lower[..index].trim();
            if let Some(number) = prefix
                .split_whitespace()
                .next_back()
                .and_then(|part| part.parse::<u32>().ok())
            {
                return Some(number);
            }
        }
    }
    None
}

fn year_of(text: &str) -> String {
    text.split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4 && part.starts_with(|c: char| c == '1' || c == '2'))
        .unwrap_or_default()
        .to_owned()
}

fn is_explicit(renderer: &Value) -> bool {
    let badges = collect(renderer, "badges");
    for badge in badges {
        let Some(items) = badge.as_array() else {
            continue;
        };
        for item in items {
            if let Some(badge) = item.get("musicInlineBadgeRenderer")
                && badge
                    .get("icon")
                    .and_then(|icon| icon.get("iconType"))
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind.contains("EXPLICIT"))
            {
                return true;
            }
            let label = text(item).to_ascii_lowercase();
            if label.contains("explicit") {
                return true;
            }
        }
    }
    false
}

fn artist_from_run(run: &Value) -> Option<Artist> {
    let id = run_browse_id(run)?;
    if !is_artist_browse_id(&id) {
        return None;
    }
    Some(Artist {
        id,
        name: run_text(run).trim().to_owned(),
        thumbnails: Vec::new(),
    })
}

fn album_from_run(run: &Value, thumbnails: &[Thumbnail]) -> Option<Album> {
    let id = run_browse_id(run)?;
    if !is_album_browse_id(&id) {
        return None;
    }
    Some(Album {
        id,
        name: run_text(run),
        thumbnails: thumbnails.to_vec(),
        ..Album::default()
    })
}

fn is_type_word(text: &str) -> bool {
    TYPE_WORDS
        .iter()
        .any(|word| word.eq_ignore_ascii_case(text.trim()))
}

// ---------------------------------------------------------------------------
// tracks
// ---------------------------------------------------------------------------

/// one `musicResponsiveListItemRenderer`, as a track.
fn flex_text(column: &Value) -> Value {
    column
        .get("musicResponsiveListItemFlexColumnRenderer")
        .and_then(|renderer| renderer.get("text"))
        .cloned()
        .unwrap_or_else(|| text_value(column))
}

fn fixed_text(column: &Value) -> Value {
    column
        .get("musicResponsiveListItemFixedColumnRenderer")
        .and_then(|renderer| renderer.get("text"))
        .cloned()
        .unwrap_or_else(|| text_value(column))
}

fn text_value(value: &Value) -> Value {
    match value {
        Value::Object(map) if map.contains_key("runs") || map.contains_key("simpleText") => {
            value.clone()
        }
        _ => Value::Null,
    }
}

pub fn parse_track_renderer(renderer: &Value, fallback_album: Option<&Album>) -> Option<Track> {
    let id = video_id(renderer)?;
    let columns = renderer.get("flexColumns").and_then(Value::as_array);
    let title = columns
        .and_then(|columns| columns.first())
        .map(|column| text(&flex_text(column)))
        .unwrap_or_default();
    if title.trim().is_empty() {
        return None;
    }

    let subtitle = columns
        .and_then(|columns| columns.get(1))
        .map(flex_text)
        .unwrap_or(Value::Null);
    let subtitle_runs = runs(&subtitle);
    let subtitle_text = text(&subtitle);

    let mut artists = Vec::new();
    let mut album = fallback_album.cloned();
    let mut duration = None;
    for run in &subtitle_runs {
        if let Some(artist) = artist_from_run(run) {
            if !artists.iter().any(|known: &Artist| known.id == artist.id) {
                artists.push(artist);
            }
            continue;
        }
        if album.is_none()
            && let Some(found) = album_from_run(run, &thumbnails(renderer))
        {
            album = Some(found);
        }
        if duration.is_none() {
            duration = clock_seconds(&run_text(run));
        }
    }

    if artists.is_empty() {
        for part in subtitle_text.split('•') {
            let part = part.trim();
            if !part.is_empty() && !is_type_word(part) && clock_seconds(part).is_none() {
                artists.push(Artist {
                    id: String::new(),
                    name: part.to_owned(),
                    thumbnails: Vec::new(),
                });
                break;
            }
        }
    }

    let duration = duration
        .or_else(|| {
            renderer
                .get("fixedColumns")
                .and_then(Value::as_array)
                .and_then(|columns| columns.first())
                .map(|column| text(&fixed_text(column)))
                .and_then(|text| clock_seconds(&text))
        })
        .unwrap_or(0);

    let track_number = renderer
        .get("index")
        .and_then(|index| index.get("runs"))
        .and_then(|runs| runs.as_array())
        .and_then(|runs| runs.first())
        .and_then(|run| run.get("text"))
        .and_then(Value::as_str)
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);

    Some(Track {
        id,
        title: title.trim().to_owned(),
        artists,
        album,
        duration_ms: duration.saturating_mul(1000),
        explicit: is_explicit(renderer),
        thumbnails: thumbnails(renderer),
        set_video_id: set_video_id(renderer),
        track_number,
    })
}

pub fn parse_tracks(root: &Value, fallback_album: Option<&Album>) -> Vec<Track> {
    let mut tracks = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for renderer in collect(root, "musicResponsiveListItemRenderer") {
        if let Some(track) = parse_track_renderer(renderer, fallback_album)
            && seen.insert((track.id.clone(), track.set_video_id.clone()))
        {
            tracks.push(track);
        }
    }
    tracks
}

/// the tracks of a playlist page, taken from its playlist shelf alone so a
/// "you might like" shelf does not end up in the playlist.
pub fn parse_playlist_shelf_tracks(root: &Value) -> Vec<Track> {
    let shelves = collect(root, "musicPlaylistShelfRenderer");
    let source: Vec<&Value> = if shelves.is_empty() {
        collect(root, "musicResponsiveListItemRenderer")
    } else {
        shelves
            .into_iter()
            .flat_map(|shelf| collect(shelf, "musicResponsiveListItemRenderer"))
            .collect()
    };
    let mut tracks = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for renderer in source {
        if let Some(track) = parse_track_renderer(renderer, None)
            && seen.insert((track.id.clone(), track.set_video_id.clone()))
        {
            tracks.push(track);
        }
    }
    tracks
}

// ---------------------------------------------------------------------------
// two-row cards: albums, artists, playlists
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Card {
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
}

pub fn parse_card(renderer: &Value, profile_id: &str) -> Option<Card> {
    let id = browse_id(renderer)?;
    let title = text(&renderer.get("title").cloned().unwrap_or(Value::Null))
        .trim()
        .to_owned();
    let subtitle_value = renderer.get("subtitle").cloned().unwrap_or(Value::Null);
    card_from_parts(&id, &title, &subtitle_value, thumbnails(renderer), renderer, profile_id)
}

/// a search result card is a `musicResponsiveListItemRenderer` with the same
/// columns a song has, but a browse id instead of a video id.
fn parse_responsive_card(renderer: &Value, profile_id: &str) -> Option<Card> {
    let id = browse_id(renderer)?;
    let columns = renderer.get("flexColumns").and_then(Value::as_array);
    let title = columns
        .and_then(|columns| columns.first())
        .map(|column| text(&flex_text(column)))
        .unwrap_or_default()
        .trim()
        .to_owned();
    let subtitle_value = columns
        .and_then(|columns| columns.get(1))
        .map(flex_text)
        .unwrap_or(Value::Null);
    card_from_parts(&id, &title, &subtitle_value, thumbnails(renderer), renderer, profile_id)
}

fn card_from_parts(
    id: &str,
    title: &str,
    subtitle_value: &Value,
    pictures: Vec<Thumbnail>,
    menu_source: &Value,
    profile_id: &str,
) -> Option<Card> {
    if title.is_empty() {
        return None;
    }
    let subtitle_runs = runs(subtitle_value);
    let subtitle = text(subtitle_value);

    if is_album_browse_id(id) {
        let artists: Vec<Artist> = subtitle_runs
            .iter()
            .filter_map(|run| artist_from_run(run))
            .collect();
        let track_count = number_before(&subtitle, &["song", "track"]).unwrap_or(0);
        return Some(Card::Album(Album {
            id: id.to_owned(),
            name: title.to_owned(),
            artists,
            thumbnails: pictures,
            release_date: year_of(&subtitle),
            track_count,
            audio_playlist_id: String::new(),
        }));
    }

    if is_artist_browse_id(id) {
        return Some(Card::Artist(Artist {
            id: id.to_owned(),
            name: title.to_owned(),
            thumbnails: pictures,
        }));
    }

    let playlist_id = id.strip_prefix("VL").unwrap_or(id).to_owned();
    if playlist_id.is_empty() {
        return None;
    }
    let track_count = number_before(&subtitle, &["song", "track"]).unwrap_or(0);
    let mut owner_name = String::new();
    for run in subtitle_runs.iter().skip(1) {
        let text = run_text(run);
        let text = text.trim();
        if !text.is_empty()
            && text.chars().any(|character| character.is_alphanumeric())
            && !is_type_word(text)
            && number_before(text, &["song", "track"]).is_none()
            && !text.eq_ignore_ascii_case("youtube music")
        {
            owner_name = text.to_owned();
            break;
        }
    }
    let owned = menu_offers_edit(menu_source)
        || owner_name.eq_ignore_ascii_case("you")
        || (profile_id.is_empty() && owner_name.is_empty());
    let owner = Owner {
        id: if owned {
            profile_id.to_owned()
        } else {
            "youtube-music".to_owned()
        },
        name: if owner_name.is_empty() {
            if owned {
                "You".to_owned()
            } else {
                "YouTube Music".to_owned()
            }
        } else {
            owner_name
        },
    };
    Some(Card::Playlist(Playlist {
        id: playlist_id,
        name: title.to_owned(),
        description: subtitle,
        thumbnails: pictures,
        owner: Some(owner),
        track_count,
        public: true,
        editable: owned,
        system: String::new(),
    }))
}

fn menu_offers_edit(renderer: &Value) -> bool {
    for menu in collect(renderer, "menu") {
        if !collect(menu, "playlistEditorEndpoint").is_empty()
            || !collect(menu, "playlistEditEndpoint").is_empty()
        {
            return true;
        }
        if contains_word(menu, "delete playlist") || contains_word(menu, "edit playlist") {
            return true;
        }
    }
    false
}

fn contains_word(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.to_ascii_lowercase().contains(needle),
        Value::Object(map) => map.values().any(|child| contains_word(child, needle)),
        Value::Array(items) => items.iter().any(|child| contains_word(child, needle)),
        _ => false,
    }
}

pub fn parse_cards(root: &Value, profile_id: &str) -> Vec<Card> {
    let mut cards = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |card: Option<Card>, cards: &mut Vec<Card>| {
        let Some(card) = card else { return };
        let id = match &card {
            Card::Album(album) => format!("a:{}", album.id),
            Card::Artist(artist) => format!("r:{}", artist.id),
            Card::Playlist(playlist) => format!("p:{}", playlist.id),
        };
        if seen.insert(id) {
            cards.push(card);
        }
    };
    for renderer in collect(root, "musicTwoRowItemRenderer") {
        push(parse_card(renderer, profile_id), &mut cards);
    }
    for renderer in collect(root, "musicResponsiveListItemRenderer") {
        push(parse_responsive_card(renderer, profile_id), &mut cards);
    }
    cards
}

// ---------------------------------------------------------------------------
// headers
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct Header {
    pub title: String,
    pub description: String,
    pub thumbnails: Vec<Thumbnail>,
    pub artists: Vec<Artist>,
    pub owner_name: String,
    pub year: String,
    pub track_count: u32,
    pub editable: bool,
    pub audio_playlist_id: String,
}

pub fn parse_header(root: &Value) -> Header {
    let mut header = Header::default();
    let editable = !collect(root, "musicEditablePlaylistDetailHeaderRenderer").is_empty();
    header.editable = editable;

    let candidates = [
        "musicDetailHeaderRenderer",
        "musicResponsiveHeaderRenderer",
        "musicEditablePlaylistDetailHeaderRenderer",
        "musicImmersiveHeaderRenderer",
        "musicVisualHeaderRenderer",
        "musicHeaderRenderer",
        "musicPageHeaderRenderer",
    ];
    let mut chosen: Option<&Value> = None;
    for key in candidates {
        if let Some(found) = first(root, key) {
            chosen = Some(found);
            break;
        }
    }
    let Some(header_value) = chosen else {
        return header;
    };

    // an editable playlist wraps the real header one level down
    let header_value = if let Some(inner) = first(header_value, "musicResponsiveHeaderRenderer")
        .or_else(|| first(header_value, "musicDetailHeaderRenderer"))
    {
        inner
    } else {
        header_value
    };

    header.title = text(&header_value.get("title").cloned().unwrap_or(Value::Null))
        .trim()
        .to_owned();
    if header.title.is_empty() {
        header.title = text(header_value).trim().to_owned();
    }
    header.description = text(&header_value.get("description").cloned().unwrap_or(Value::Null));
    if header.description.is_empty()
        && let Some(shelf) = first(header_value, "musicDescriptionShelfRenderer")
    {
        header.description = text(&shelf.get("description").cloned().unwrap_or(Value::Null));
    }
    header.thumbnails = thumbnails(header_value);
    if header.thumbnails.is_empty()
        && let Some(thumbnail) = first(header_value, "foregroundThumbnail")
    {
        header.thumbnails = thumbnails(thumbnail);
    }

    let subtitle_value = header_value
        .get("subtitle")
        .or_else(|| header_value.get("straplineTextOne"))
        .cloned()
        .unwrap_or(Value::Null);
    let subtitle = text(&subtitle_value);
    for run in runs(&subtitle_value) {
        if let Some(artist) = artist_from_run(run) {
            if !header.artists.iter().any(|known| known.id == artist.id) {
                header.artists.push(artist);
            }
        }
    }
    if header.artists.is_empty() {
        for part in subtitle.split('•') {
            let part = part.trim();
            if !part.is_empty() && !is_type_word(part) && clock_seconds(part).is_none() {
                header.artists.push(Artist {
                    id: String::new(),
                    name: part.to_owned(),
                    thumbnails: Vec::new(),
                });
                break;
            }
        }
    }

    let second = header_value
        .get("secondSubtitle")
        .cloned()
        .unwrap_or(Value::Null);
    let second_text = text(&second);
    header.track_count = number_before(&second_text, &["song", "track"])
        .or_else(|| number_before(&subtitle, &["song", "track"]))
        .unwrap_or(0);
    header.year = year_of(&subtitle);
    if header.year.is_empty() {
        header.year = year_of(&second_text);
    }
    header.owner_name = subtitle
        .split('•')
        .map(str::trim)
        .find(|part| !part.is_empty() && !is_type_word(part) && clock_seconds(part).is_none())
        .unwrap_or_default()
        .to_owned();
    header.audio_playlist_id = first(header_value, "audioPlaylistId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    header
}

// ---------------------------------------------------------------------------
// whole pages
// ---------------------------------------------------------------------------

pub fn parse_playlist_page(root: &Value, requested_id: &str, profile_id: &str) -> (Playlist, Vec<Track>) {
    let header = parse_header(root);
    let tracks = parse_playlist_shelf_tracks(root);
    let id = requested_id.strip_prefix("VL").unwrap_or(requested_id).to_owned();
    let owned = header.editable;
    let owner = if owned {
        Owner {
            id: profile_id.to_owned(),
            name: "You".to_owned(),
        }
    } else {
        Owner {
            id: "youtube-music".to_owned(),
            name: if header.owner_name.is_empty() {
                "YouTube Music".to_owned()
            } else {
                header.owner_name.clone()
            },
        }
    };
    let playlist = Playlist {
        id: id.clone(),
        name: if header.title.is_empty() {
            "Playlist".to_owned()
        } else {
            header.title
        },
        description: header.description,
        thumbnails: header.thumbnails,
        owner: Some(owner),
        track_count: if header.track_count > 0 {
            header.track_count
        } else {
            tracks.len() as u32
        },
        public: true,
        editable: owned,
        system: id,
    };
    (playlist, tracks)
}

pub fn parse_album_page(root: &Value, requested_id: &str) -> (Album, Vec<Track>) {
    let header = parse_header(root);
    let mut album = Album {
        id: requested_id.to_owned(),
        name: header.title,
        artists: header.artists,
        thumbnails: header.thumbnails,
        release_date: header.year,
        track_count: header.track_count,
        audio_playlist_id: header.audio_playlist_id,
    };
    let tracks = parse_tracks(root, Some(&album));
    if album.track_count == 0 {
        album.track_count = tracks.len() as u32;
    }
    if album.name.is_empty() {
        album.name = "Album".to_owned();
    }
    (album, tracks)
}

/// the artist page's "Top songs" title links to the playlist holding every
/// song, not only the five the shelf shows.
pub fn artist_songs_browse_id(root: &Value) -> Option<String> {
    for shelf in collect(root, "musicShelfRenderer") {
        let title = shelf.get("title").cloned().unwrap_or(Value::Null);
        if !text(&title).to_ascii_lowercase().contains("song") {
            continue;
        }
        if let Some(id) = title
            .pointer("/runs/0/navigationEndpoint/browseEndpoint/browseId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        {
            return Some(id.to_owned());
        }
    }
    None
}

pub fn parse_artist_page(root: &Value, requested_id: &str) -> (Artist, Vec<Track>, Vec<Album>) {
    let header = parse_header(root);
    let artist = Artist {
        id: requested_id.to_owned(),
        name: if header.title.is_empty() {
            "Artist".to_owned()
        } else {
            header.title
        },
        thumbnails: header.thumbnails,
    };
    let tracks = parse_tracks(root, None);
    let albums = parse_cards(root, "")
        .into_iter()
        .filter_map(|card| match card {
            Card::Album(album) => Some(album),
            _ => None,
        })
        .collect();
    (artist, tracks, albums)
}

pub fn parse_library_playlists(root: &Value, profile_id: &str) -> Vec<Playlist> {
    parse_cards(root, profile_id)
        .into_iter()
        .filter_map(|card| match card {
            Card::Playlist(playlist) => Some(playlist),
            _ => None,
        })
        .filter(|playlist| !playlist.name.is_empty())
        .collect()
}

pub fn parse_history(root: &Value) -> Vec<Track> {
    parse_tracks(root, None)
}

// ---------------------------------------------------------------------------
// continuations
// ---------------------------------------------------------------------------

pub fn continuation_token(root: &Value) -> Option<String> {
    for item in collect(root, "continuationItemRenderer") {
        if let Some(token) = item
            .pointer("/continuationEndpoint/continuationCommand/token")
            .and_then(Value::as_str)
            && !token.is_empty()
        {
            return Some(token.to_owned());
        }
    }
    for data in collect(root, "nextContinuationData") {
        if let Some(token) = data.get("continuation").and_then(Value::as_str)
            && !token.is_empty()
        {
            return Some(token.to_owned());
        }
    }
    for command in collect(root, "continuationCommand") {
        if let Some(token) = command.get("token").and_then(Value::as_str)
            && !token.is_empty()
        {
            return Some(token.to_owned());
        }
    }
    None
}

/// the continuation belonging to the shelf that holds tracks, not to a
/// related shelf beside it.
pub fn shelf_continuation(root: &Value) -> Option<String> {
    for key in ["musicPlaylistShelfRenderer", "musicShelfRenderer", "gridRenderer"] {
        for shelf in collect(root, key) {
            if let Some(token) = continuation_token(shelf) {
                return Some(token);
            }
        }
    }
    continuation_token(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clock_reads_minutes_and_hours() {
        assert_eq!(clock_seconds("3:42"), Some(222));
        assert_eq!(clock_seconds("1:02:03"), Some(3723));
        assert_eq!(clock_seconds("Song"), None);
        assert_eq!(clock_seconds("12"), None);
    }

    #[test]
    fn a_song_renderer_becomes_a_track() {
        let renderer = json!({
            "playlistItemData": { "videoId": "abc123", "setVideoId": "set1" },
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "One More Time" }] } } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                    { "text": "Daft Punk", "navigationEndpoint": { "browseEndpoint": { "browseId": "UC1" } } },
                    { "text": " • " },
                    { "text": "Discovery", "navigationEndpoint": { "browseEndpoint": { "browseId": "MPREb_1" } } },
                    { "text": " • " },
                    { "text": "5:20" }
                ] } } }
            ],
            "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://img/1.jpg", "width": 60, "height": 60 },
                { "url": "https://img/2.jpg", "width": 544, "height": 544 }
            ] } } }
        });
        let track = parse_track_renderer(&renderer, None).unwrap();
        assert_eq!(track.id, "abc123");
        assert_eq!(track.title, "One More Time");
        assert_eq!(track.artists[0].name, "Daft Punk");
        assert_eq!(track.album.as_ref().unwrap().name, "Discovery");
        assert_eq!(track.duration_ms, 320_000);
        assert_eq!(track.set_video_id, "set1");
        assert_eq!(track.thumbnails.len(), 2);
    }

    #[test]
    fn a_playlist_card_keeps_its_id_and_count() {
        let renderer = json!({
            "title": { "runs": [{ "text": "Late night" }] },
            "subtitle": { "runs": [{ "text": "Playlist" }, { "text": " • " }, { "text": "You" }, { "text": " • " }, { "text": "42 songs" }] },
            "thumbnailRenderer": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [{ "url": "https://img/p.jpg", "width": 300, "height": 300 }] } } },
            "navigationEndpoint": { "browseEndpoint": { "browseId": "VLPL123" } }
        });
        let Card::Playlist(playlist) = parse_card(&renderer, "me").unwrap() else {
            panic!("not a playlist");
        };
        assert_eq!(playlist.id, "PL123");
        assert_eq!(playlist.name, "Late night");
        assert_eq!(playlist.track_count, 42);
        assert_eq!(playlist.owner.as_ref().unwrap().id, "me");
    }

    #[test]
    fn an_album_header_reads_the_year_and_count() {
        let root = json!({
            "header": { "musicResponsiveHeaderRenderer": {
                "title": { "runs": [{ "text": "Discovery" }] },
                "subtitle": { "runs": [{ "text": "Daft Punk", "navigationEndpoint": { "browseEndpoint": { "browseId": "UC1" } } }] },
                "secondSubtitle": { "runs": [{ "text": "14 songs" }, { "text": " • " }, { "text": "1 hour" }] },
                "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [{ "url": "https://img/a.jpg", "width": 500, "height": 500 }] } } }
            } }
        });
        let header = parse_header(&root);
        assert_eq!(header.title, "Discovery");
        assert_eq!(header.track_count, 14);
        assert_eq!(header.artists[0].name, "Daft Punk");
    }
}
