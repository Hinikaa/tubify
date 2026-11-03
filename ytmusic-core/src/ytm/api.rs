//! every library operation the app asks for, on top of the innertube client.
//!
//! the answers are the shapes the qt layer already parses (see `models`), so
//! swapping the service did not mean rewriting the views.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::{
    auth::Auth,
    client::{
        Innertube, SEARCH_ALBUMS, SEARCH_ARTISTS, SEARCH_PLAYLISTS, SEARCH_SONGS,
    },
    lyrics,
    models::{
        Album, Artist, Playlist, Profile, Track, album_page, id_from_uri,
        playlist_browse_id, playlist_page, track_page,
    },
    parse::{
        Card, parse_album_page, parse_artist_page, parse_cards, parse_history,
        parse_library_playlists, parse_playlist_page, parse_playlist_shelf_tracks, parse_tracks,
        shelf_continuation,
    },
};

const LIBRARY_PLAYLISTS: &str = "FEmusic_liked_playlists";
const LIKED_MUSIC: &str = "LM";
const HISTORY: &str = "FEmusic_history";
const MAX_TRACKS: usize = 5_000;

#[derive(Clone, Debug)]
struct ArtistPage {
    artist: Artist,
    tracks: Vec<Track>,
    albums: Vec<Album>,
    songs_browse_id: Option<String>,
    fetched_at: Instant,
}

pub struct Api {
    client: Innertube,
    pub profile: Profile,
    liked: Mutex<Option<(HashSet<String>, Instant)>>,
    artists: Mutex<HashMap<String, ArtistPage>>,
    albums: Mutex<HashMap<String, (Album, Vec<Track>, Instant)>>,
    playlists: Mutex<HashMap<String, (Playlist, Vec<Track>, Instant)>>,
}

impl Api {
    pub fn new(auth: Auth, network_filter: bool) -> Result<Self, String> {
        let profile = auth.profile();
        let client = Innertube::new(Some(auth), network_filter)?;
        Ok(Self {
            client,
            profile,
            liked: Mutex::new(None),
            artists: Mutex::new(HashMap::new()),
            albums: Mutex::new(HashMap::new()),
            playlists: Mutex::new(HashMap::new()),
        })
    }

    pub fn set_network_filter(&self, enabled: bool) {
        self.client.set_network_filter(enabled);
    }

    pub fn auth(&self) -> Option<&Auth> {
        self.client.auth()
    }

    // --- library ----------------------------------------------------------

    pub async fn profile_json(&self) -> Value {
        self.profile.to_json(512)
    }

    pub async fn playlists(&self) -> Result<Vec<Playlist>, String> {
        let response = self.client.browse(LIBRARY_PLAYLISTS, None, None).await?;
        let mut playlists = parse_library_playlists(&response, &self.profile.id);
        let mut token = shelf_continuation(&response);
        let mut pages = 0;
        while let Some(continuation) = token {
            pages += 1;
            if pages > 40 {
                break;
            }
            let response = self
                .client
                .browse(LIBRARY_PLAYLISTS, None, Some(&continuation))
                .await?;
            let more = parse_library_playlists(&response, &self.profile.id);
            if more.is_empty() {
                break;
            }
            playlists.extend(more);
            token = shelf_continuation(&response);
        }
        let mut seen = HashSet::new();
        playlists.retain(|playlist| seen.insert(playlist.id.clone()));
        Ok(playlists)
    }

    pub async fn playlists_json(&self) -> Result<Value, String> {
        Ok(playlist_page(&self.playlists().await?))
    }

    /// the playlist's header and every track in it.
    pub async fn playlist(&self, id: &str) -> Result<(Playlist, Vec<Track>), String> {
        let id = id_from_uri(id);
        let id = id.strip_prefix("VL").unwrap_or(&id).to_owned();
        if let Some((playlist, tracks, at)) = self.playlists.lock().await.get(&id).cloned()
            && at.elapsed() < Duration::from_secs(45)
        {
            return Ok((playlist, tracks));
        }

        let browse_id = playlist_browse_id(&id);
        let response = self.client.browse(&browse_id, None, None).await?;
        let (mut playlist, mut tracks) = parse_playlist_page(&response, &id, &self.profile.id);
        let mut token = shelf_continuation(&response);
        let mut pages = 0;
        while let Some(continuation) = token {
            pages += 1;
            if pages > 60 || tracks.len() >= MAX_TRACKS {
                break;
            }
            let response = self
                .client
                .browse(&browse_id, None, Some(&continuation))
                .await?;
            let more = parse_playlist_shelf_tracks(&response);
            if more.is_empty() {
                break;
            }
            tracks.extend(more);
            token = shelf_continuation(&response);
        }
        dedupe_tracks(&mut tracks);
        if playlist.track_count == 0 {
            playlist.track_count = tracks.len() as u32;
        } else {
            tracks.truncate(playlist.track_count as usize);
        }
        self.playlists.lock().await.insert(
            id,
            (playlist.clone(), tracks.clone(), Instant::now()),
        );
        Ok((playlist, tracks))
    }

    pub async fn playlist_json(&self, id: &str) -> Result<Value, String> {
        Ok(self.playlist(id).await?.0.to_json(640))
    }

    pub async fn playlist_songs(&self, id: &str) -> Result<Vec<Track>, String> {
        Ok(self.playlist(id).await?.1)
    }

    pub async fn playlist_songs_json(&self, id: &str) -> Result<Value, String> {
        let (playlist, tracks) = self.playlist(id).await?;
        Ok(track_page(&tracks, playlist.track_count as usize))
    }

    pub async fn saved_songs(&self) -> Result<(Playlist, Vec<Track>), String> {
        self.playlist(LIKED_MUSIC).await
    }

    pub async fn saved_songs_json(&self) -> Result<Value, String> {
        let (playlist, tracks) = self.saved_songs().await?;
        Ok(track_page(&tracks, playlist.track_count as usize))
    }

    pub async fn saved_total_json(&self) -> Result<Value, String> {
        let (playlist, tracks) = self.saved_songs().await?;
        let total = if playlist.track_count > 0 {
            playlist.track_count
        } else {
            tracks.len() as u32
        };
        Ok(json!({ "total": total }))
    }

    pub async fn recently_played(&self) -> Result<Vec<Track>, String> {
        let response = self.client.browse(HISTORY, None, None).await?;
        let mut tracks = parse_history(&response);
        dedupe_tracks(&mut tracks);
        tracks.truncate(40);
        Ok(tracks)
    }

    pub async fn recently_played_json(&self) -> Result<Value, String> {
        let tracks = self.recently_played().await?;
        Ok(json!({
            "items": tracks.iter().map(|track| json!({ "item": track.to_json(640) })).collect::<Vec<_>>(),
        }))
    }

    // --- search -----------------------------------------------------------

    pub async fn search(&self, query: &str) -> Result<Value, String> {
        let (songs, albums, artists, playlists) = tokio::join!(
            self.client.search(query, Some(SEARCH_SONGS)),
            self.client.search(query, Some(SEARCH_ALBUMS)),
            self.client.search(query, Some(SEARCH_ARTISTS)),
            self.client.search(query, Some(SEARCH_PLAYLISTS)),
        );

        let songs = songs.map(|root| {
            let mut tracks = parse_tracks(&root, None);
            dedupe_tracks(&mut tracks);
            tracks.truncate(20);
            tracks
        })?;
        let albums = albums.map(|root| {
            parse_cards(&root, &self.profile.id)
                .into_iter()
                .filter_map(|card| match card {
                    Card::Album(album) => Some(album),
                    _ => None,
                })
                .take(20)
                .collect::<Vec<_>>()
        })?;
        let artists = artists.map(|root| {
            parse_cards(&root, &self.profile.id)
                .into_iter()
                .filter_map(|card| match card {
                    Card::Artist(artist) => Some(artist),
                    _ => None,
                })
                .take(20)
                .collect::<Vec<_>>()
        })?;
        let playlists = playlists.map(|root| {
            parse_cards(&root, &self.profile.id)
                .into_iter()
                .filter_map(|card| match card {
                    Card::Playlist(playlist) => Some(playlist),
                    _ => None,
                })
                .take(20)
                .collect::<Vec<_>>()
        })?;

        Ok(json!({
            "tracks": { "items": songs.iter().map(|track| track.to_json(640)).collect::<Vec<_>>() },
            "albums": { "items": albums.iter().map(|album| album.to_json(640)).collect::<Vec<_>>() },
            "artists": { "items": artists.iter().map(|artist| artist.to_json(640)).collect::<Vec<_>>() },
            "playlists": { "items": playlists.iter().map(|playlist| playlist.to_json(640)).collect::<Vec<_>>() },
        }))
    }

    // --- albums and artists ----------------------------------------------

    pub async fn album(&self, id: &str) -> Result<(Album, Vec<Track>), String> {
        let id = id_from_uri(id);
        if let Some((album, tracks, at)) = self.albums.lock().await.get(&id).cloned()
            && at.elapsed() < Duration::from_secs(45)
        {
            return Ok((album, tracks));
        }
        let response = self.client.browse(&id, None, None).await?;
        let (mut album, mut tracks) = parse_album_page(&response, &id);
        let mut token = shelf_continuation(&response);
        let mut pages = 0;
        while let Some(continuation) = token {
            pages += 1;
            if pages > 20 || tracks.len() >= MAX_TRACKS {
                break;
            }
            let response = self.client.browse(&id, None, Some(&continuation)).await?;
            let more = parse_tracks(&response, Some(&album));
            if more.is_empty() {
                break;
            }
            tracks.extend(more);
            token = shelf_continuation(&response);
        }
        dedupe_tracks(&mut tracks);
        if album.track_count == 0 {
            album.track_count = tracks.len() as u32;
        }
        self.albums
            .lock()
            .await
            .insert(id, (album.clone(), tracks.clone(), Instant::now()));
        Ok((album, tracks))
    }

    pub async fn album_json(&self, id: &str) -> Result<Value, String> {
        Ok(self.album(id).await?.0.to_json(640))
    }

    pub async fn album_songs_json(&self, id: &str) -> Result<Value, String> {
        let (album, tracks) = self.album(id).await?;
        Ok(track_page(&tracks, album.track_count as usize))
    }

    async fn artist_page(&self, id: &str) -> Result<ArtistPage, String> {
        let id = id_from_uri(id);
        if let Some(page) = self.artists.lock().await.get(&id).cloned()
            && page.fetched_at.elapsed() < Duration::from_secs(45)
        {
            return Ok(page);
        }
        let response = self.client.browse(&id, None, None).await?;
        let (artist, tracks, albums) = parse_artist_page(&response, &id);
        let page = ArtistPage {
            artist,
            tracks,
            albums,
            songs_browse_id: super::parse::artist_songs_browse_id(&response),
            fetched_at: Instant::now(),
        };
        self.artists.lock().await.insert(id, page.clone());
        Ok(page)
    }

    pub async fn artist_json(&self, id: &str) -> Result<Value, String> {
        Ok(self.artist_page(id).await?.artist.to_json(640))
    }

    pub async fn artist_albums_json(&self, id: &str) -> Result<Value, String> {
        Ok(album_page(&self.artist_page(id).await?.albums))
    }

    pub async fn artist_top_tracks_json(&self, id: &str) -> Result<Value, String> {
        let page = self.artist_page(id).await?;
        let tracks = if let Some(browse_id) = &page.songs_browse_id {
            match self.playlist(browse_id).await {
                Ok((_, all)) if !all.is_empty() => all,
                _ => page.tracks,
            }
        } else {
            page.tracks
        };
        Ok(json!({
            "tracks": tracks.iter().map(|track| track.to_json(640)).collect::<Vec<_>>(),
        }))
    }

    /// youtube music does not publish an artist's genres. the stats page asks
    /// anyway; it gets the artist's picture and an empty genre list.
    pub async fn artist_genres_json(&self, ids: &[String]) -> Value {
        let artists = self.artists.lock().await;
        let known: Vec<Value> = ids
            .iter()
            .filter_map(|id| {
                let id = id_from_uri(id);
                artists.get(&id).map(|page| {
                    json!({
                        "id": page.artist.id,
                        "genres": [],
                        "images": super::models::images_json(&page.artist.thumbnails, 320),
                    })
                })
            })
            .collect();
        json!({ "artists": known })
    }

    pub async fn song_json(&self, id: &str) -> Result<Value, String> {
        let id = id_from_uri(id);
        let response = self.client.next(&id).await?;
        let track = parse_next_track(&response, &id)
            .ok_or_else(|| "YouTube Music did not describe that song.".to_owned())?;
        Ok(track.to_json(640))
    }

    // --- changes ----------------------------------------------------------

    pub async fn set_saved(&self, saved: bool, uri: &str) -> Result<(), String> {
        let id = id_from_uri(uri);
        let endpoint = if saved { "like/like" } else { "like/removelike" };
        self.client
            .mutate(endpoint, json!({ "target": { "videoId": id } }))
            .await?;
        if let Some((ids, _)) = self.liked.lock().await.as_mut() {
            if saved {
                ids.insert(id);
            } else {
                ids.remove(&id);
            }
        }
        Ok(())
    }

    async fn liked_ids(&self) -> Result<HashSet<String>, String> {
        {
            let cache = self.liked.lock().await;
            if let Some((ids, at)) = cache.as_ref()
                && at.elapsed() < Duration::from_secs(120)
            {
                return Ok(ids.clone());
            }
        }
        let (_, tracks) = self.saved_songs().await?;
        let ids: HashSet<String> = tracks.into_iter().map(|track| track.id).collect();
        *self.liked.lock().await = Some((ids.clone(), Instant::now()));
        Ok(ids)
    }

    pub async fn saved_statuses_json(&self, uris: &[String]) -> Result<Value, String> {
        let ids = self.liked_ids().await?;
        let mut map = serde_json::Map::new();
        for uri in uris {
            map.insert(uri.clone(), Value::Bool(ids.contains(&id_from_uri(uri))));
        }
        Ok(Value::Object(map))
    }

    pub async fn create_playlist(
        &self,
        title: &str,
        description: &str,
        public: bool,
    ) -> Result<Value, String> {
        let response = self
            .client
            .mutate(
                "playlist/create",
                json!({
                    "title": title,
                    "description": description,
                    "privacy": if public { "PUBLIC" } else { "PRIVATE" },
                }),
            )
            .await?;
        let id = response
            .get("playlistId")
            .and_then(Value::as_str)
            .ok_or_else(|| "YouTube Music did not return the new playlist's id.".to_owned())?;
        let playlist = Playlist {
            id: id.to_owned(),
            name: title.to_owned(),
            description: description.to_owned(),
            owner: Some(super::models::Owner {
                id: self.profile.id.clone(),
                name: self.profile.name.clone(),
            }),
            track_count: 0,
            public,
            editable: true,
            ..Playlist::default()
        };
        Ok(playlist.to_json(640))
    }

    pub async fn update_playlist(
        &self,
        id: &str,
        title: &str,
        description: &str,
        public: bool,
    ) -> Result<(), String> {
        let id = id_from_uri(id);
        let id = id.strip_prefix("VL").unwrap_or(&id);
        self.client
            .mutate(
                "playlist/edit",
                json!({
                    "playlistId": id,
                    "actions": [
                        { "action": "ACTION_SET_PLAYLIST_NAME", "playlistName": title },
                        { "action": "ACTION_SET_PLAYLIST_DESCRIPTION", "playlistDescription": description },
                        { "action": "ACTION_SET_PLAYLIST_PRIVACY", "playlistPrivacy": if public { "PUBLIC" } else { "PRIVATE" } },
                    ],
                }),
            )
            .await?;
        self.playlists.lock().await.remove(id);
        Ok(())
    }

    pub async fn add_to_playlist(&self, uri: &str, playlist_id: &str) -> Result<(), String> {
        let video_id = id_from_uri(uri);
        let playlist_id = id_from_uri(playlist_id);
        let playlist_id = playlist_id.strip_prefix("VL").unwrap_or(&playlist_id);
        self.client
            .mutate(
                "browse/edit_playlist",
                json!({
                    "playlistId": playlist_id,
                    "actions": [{ "action": "ACTION_ADD_VIDEO", "addedVideoId": video_id }],
                }),
            )
            .await?;
        self.playlists.lock().await.remove(playlist_id);
        Ok(())
    }

    pub async fn remove_from_playlist(&self, uri: &str, playlist_id: &str) -> Result<(), String> {
        let video_id = id_from_uri(uri);
        let playlist_id = id_from_uri(playlist_id);
        let playlist_id = playlist_id.strip_prefix("VL").unwrap_or(&playlist_id);
        let (_, tracks) = self.playlist(playlist_id).await?;
        let track = tracks
            .iter()
            .find(|track| track.id == video_id && !track.set_video_id.is_empty())
            .ok_or_else(|| {
                "That song is no longer in the playlist, or YouTube Music did not say which row it is."
                    .to_owned()
            })?;
        self.client
            .mutate(
                "browse/edit_playlist",
                json!({
                    "playlistId": playlist_id,
                    "actions": [{
                        "action": "ACTION_REMOVE_VIDEO",
                        "setVideoId": track.set_video_id,
                        "removedVideoId": video_id,
                    }],
                }),
            )
            .await?;
        self.playlists.lock().await.remove(playlist_id);
        Ok(())
    }

    pub async fn reorder_playlist(
        &self,
        playlist_id: &str,
        range_start: usize,
        insert_before: usize,
        length: usize,
    ) -> Result<(), String> {
        let playlist_id = id_from_uri(playlist_id);
        let playlist_id = playlist_id.strip_prefix("VL").unwrap_or(&playlist_id);
        let (_, tracks) = self.playlist(playlist_id).await?;
        if range_start >= tracks.len() {
            return Err("That row is no longer in the playlist.".to_owned());
        }
        let mut actions = Vec::new();
        let count = length.max(1).min(tracks.len() - range_start);
        for offset in 0..count {
            let from = range_start + offset;
            let Some(track) = tracks.get(from) else {
                break;
            };
            if track.set_video_id.is_empty() {
                return Err("YouTube Music did not say which row to move.".to_owned());
            }
            let successor = tracks
                .get(insert_before + offset)
                .filter(|successor| successor.set_video_id != track.set_video_id)
                .map(|successor| successor.set_video_id.clone());
            let mut action = json!({
                "action": "ACTION_MOVE_VIDEO_BEFORE",
                "setVideoId": track.set_video_id,
            });
            if let Some(successor) = successor
                && !successor.is_empty()
            {
                action["movedSetVideoIdSuccessor"] = Value::String(successor);
            }
            actions.push(action);
        }
        if actions.is_empty() {
            return Ok(());
        }
        self.client
            .mutate(
                "browse/edit_playlist",
                json!({ "playlistId": playlist_id, "actions": actions }),
            )
            .await?;
        self.playlists.lock().await.remove(playlist_id);
        Ok(())
    }

    pub async fn delete_playlist(&self, id: &str) -> Result<(), String> {
        let id = id_from_uri(id);
        let id = id.strip_prefix("VL").unwrap_or(&id);
        self.client
            .mutate("playlist/delete", json!({ "playlistId": id }))
            .await?;
        self.playlists.lock().await.remove(id);
        Ok(())
    }

    pub async fn set_playlist_image(&self, _id: &str, _jpeg: &[u8]) -> Result<(), String> {
        Err(
            "YouTube Music does not let an app set a playlist's cover. Its own covers stay."
                .to_owned(),
        )
    }

    // --- lyrics -----------------------------------------------------------

    pub async fn lyrics_json(&self, uri: &str) -> Result<Value, String> {
        let video_id = id_from_uri(uri);
        let response = self.client.next(&video_id).await?;
        let track = parse_next_track(&response, &video_id);

        if let Some(track) = &track
            && let Some(synced) = lyrics::lrclib(
                &track.artist_line(),
                &track.title,
                Some(track.duration_ms / 1000),
            )
            .await
        {
            return Ok(synced);
        }

        if let Some(lyrics_id) = find_lyrics_browse_id(&response) {
            let page = self.client.browse(&lyrics_id, None, None).await?;
            let words = find_description(&page);
            if !words.trim().is_empty() {
                let lines: Vec<Value> = words
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| {
                        json!({
                            "startTimeMs": 0,
                            "endTimeMs": 0,
                            "words": line.trim(),
                        })
                    })
                    .collect();
                return Ok(json!({
                    "language": "en",
                    "provider": "YouTube Music",
                    "syncType": "unsynchronized",
                    "lines": lines,
                }));
            }
        }

        if let Some(track) = track
            && let Some(plain) = lyrics::lrclib_plain(
                &track.artist_line(),
                &track.title,
                Some(track.duration_ms / 1000),
            )
            .await
        {
            return Ok(plain);
        }

        Err("No lyrics were found for this song.".to_owned())
    }

    // --- dispatch ---------------------------------------------------------

    pub async fn call(&self, op: &str, args: &Value) -> Result<Value, String> {
        match op {
            "profile" => Ok(self.profile_json().await),
            "playlists" => self.playlists_json().await,
            "playlist" => self.playlist_json(arg(args, "id")).await,
            "playlistSongs" => self.playlist_songs_json(arg(args, "id")).await,
            "savedSongs" => self.saved_songs_json().await,
            "savedSongTotal" => self.saved_total_json().await,
            "recentlyPlayed" => self.recently_played_json().await,
            "search" => self.search(arg(args, "query")).await,
            "song" => self.song_json(arg(args, "id")).await,
            "album" => self.album_json(arg(args, "id")).await,
            "albumSongs" => self.album_songs_json(arg(args, "id")).await,
            "artist" => self.artist_json(arg(args, "id")).await,
            "artistAlbums" => self.artist_albums_json(arg(args, "id")).await,
            "artistTopTracks" => self.artist_top_tracks_json(arg(args, "id")).await,
            "artistGenres" => {
                let ids: Vec<String> = args
                    .get("ids")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(self.artist_genres_json(&ids).await)
            }
            "setSaved" => {
                let saved = args.get("saved").and_then(Value::as_bool).unwrap_or(true);
                self.set_saved(saved, arg(args, "uri")).await?;
                Ok(json!({}))
            }
            "savedStatuses" => {
                let uris: Vec<String> = args
                    .get("uris")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                self.saved_statuses_json(&uris).await
            }
            "createPlaylist" => {
                self.create_playlist(
                    arg(args, "name"),
                    arg(args, "description"),
                    args.get("public").and_then(Value::as_bool).unwrap_or(false),
                )
                .await
            }
            "updatePlaylist" => {
                self.update_playlist(
                    arg(args, "id"),
                    arg(args, "name"),
                    arg(args, "description"),
                    args.get("public").and_then(Value::as_bool).unwrap_or(false),
                )
                .await?;
                Ok(json!({}))
            }
            "addToPlaylist" => {
                self.add_to_playlist(arg(args, "uri"), arg(args, "playlistId"))
                    .await?;
                Ok(json!({}))
            }
            "removeFromPlaylist" => {
                self.remove_from_playlist(arg(args, "uri"), arg(args, "playlistId"))
                    .await?;
                Ok(json!({}))
            }
            "reorderPlaylist" => {
                self.reorder_playlist(
                    arg(args, "playlistId"),
                    args.get("rangeStart").and_then(Value::as_u64).unwrap_or(0) as usize,
                    args.get("insertBefore").and_then(Value::as_u64).unwrap_or(0) as usize,
                    args.get("length").and_then(Value::as_u64).unwrap_or(1) as usize,
                )
                .await?;
                Ok(json!({}))
            }
            "deletePlaylist" => {
                self.delete_playlist(arg(args, "id")).await?;
                Ok(json!({}))
            }
            "setPlaylistImage" => {
                Err("YouTube Music does not let an app set a playlist's cover.".to_owned())
            }
            "lyrics" => self.lyrics_json(arg(args, "uri")).await,
            other => Err(format!("unknown YouTube Music operation: {other}")),
        }
    }
}

fn arg<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn dedupe_tracks(tracks: &mut Vec<Track>) {
    let mut seen = HashSet::new();
    tracks.retain(|track| seen.insert((track.id.clone(), track.set_video_id.clone())));
}

/// the song the watch-next page is about: its first queue row.
fn parse_next_track(root: &Value, video_id: &str) -> Option<Track> {
    let renderer = collect_key(root, "playlistPanelVideoRenderer")
        .into_iter()
        .find(|renderer| {
            renderer.get("videoId").and_then(Value::as_str) == Some(video_id)
        })
        .or_else(|| collect_key(root, "playlistPanelVideoRenderer").into_iter().next())?;

    let title = super::parse::text(&renderer.get("title").cloned().unwrap_or(Value::Null))
        .trim()
        .to_owned();
    let byline = renderer
        .get("longBylineText")
        .or_else(|| renderer.get("shortBylineText"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut artists = Vec::new();
    let mut album = None;
    for run in byline.get("runs").and_then(Value::as_array).into_iter().flatten() {
        if let Some(id) = run
            .pointer("/navigationEndpoint/browseEndpoint/browseId")
            .and_then(Value::as_str)
        {
            let name = run.get("text").and_then(Value::as_str).unwrap_or_default();
            if id.starts_with("UC") {
                artists.push(Artist {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    thumbnails: Vec::new(),
                });
            } else if id.starts_with("MPRE") {
                album = Some(Album {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    thumbnails: super::parse::thumbnails(renderer),
                    ..Album::default()
                });
            }
        }
    }
    let duration = renderer
        .get("lengthText")
        .and_then(|length| length.get("runs"))
        .and_then(Value::as_array)
        .and_then(|runs| runs.first())
        .and_then(|run| run.get("text"))
        .and_then(Value::as_str)
        .and_then(super::parse::clock_seconds)
        .unwrap_or(0);
    Some(Track {
        id: video_id.to_owned(),
        title,
        artists,
        album,
        duration_ms: duration.saturating_mul(1000),
        explicit: false,
        thumbnails: super::parse::thumbnails(renderer),
        set_video_id: String::new(),
        track_number: 0,
    })
}

fn collect_key<'a>(value: &'a Value, key: &str) -> Vec<&'a Value> {
    super::parse::collect(value, key)
}

fn find_lyrics_browse_id(root: &Value) -> Option<String> {
    fn walk(value: &Value) -> Option<String> {
        match value {
            Value::String(text) if text.starts_with("MPLY") => Some(text.clone()),
            Value::Object(map) => map.values().find_map(walk),
            Value::Array(items) => items.iter().find_map(walk),
            _ => None,
        }
    }
    walk(root)
}

fn find_description(root: &Value) -> String {
    for shelf in super::parse::collect(root, "musicDescriptionShelfRenderer") {
        let description = shelf.get("description").cloned().unwrap_or(Value::Null);
        let text = super::parse::text(&description);
        if !text.trim().is_empty() {
            return text;
        }
    }
    String::new()
}
