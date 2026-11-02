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

//! the YouTube Music core: one login, one library, one audio output.
//!
//! songs are fetched by yt-dlp into a cache and then played from disk through
//! the same output, equalizer and analyser local files use. keeping the whole
//! pipeline on a file makes seeking exact and the visualizers identical
//! whether a song came from YouTube Music or from the person's own disk.

use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use rodio::Source;
use serde_json::Value;
use tokio::sync::{Mutex as AsyncMutex, watch};

mod dsp;
mod file_playback;
pub mod live_input;
pub mod local;
mod output;
mod spectrum;
pub mod ytm;

#[cfg(feature = "capi")]
pub mod capi;

use dsp::{EQ_BAND_COUNT, SharedDsp};
use spectrum::{OutputHealth, SharedSpectrum};
use ytm::{api::Api, auth::Auth, models::id_from_uri, stream};

/// the output files play through. replaceable, not a oncelock: a device that
/// comes and goes needs a fresh stream.
static LOCAL_AUDIO_SINK: Mutex<Option<Arc<output::LocalOutput>>> = Mutex::new(None);

fn silence_cached_local_sink() {
    let Ok(cached) = LOCAL_AUDIO_SINK.lock() else {
        return;
    };
    if let Some(sink) = cached.as_ref() {
        sink.stop();
        sink.clear();
    }
}

fn stop_cached_local_sink() {
    let Ok(mut cached) = LOCAL_AUDIO_SINK.lock() else {
        return;
    };
    let Some(sink) = cached.take() else {
        return;
    };
    sink.stop();
    sink.clear();
}

/// the bands the equalizer is built from, so the ui can label its own sliders
/// from the filters that actually run rather than a copy of the list.
pub fn equalizer_frequencies() -> Vec<f32> {
    dsp::EQ_FREQUENCIES.to_vec()
}

pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// what a play request that a newer one replaced ends with
/// (TUBIFY_CORE_SUPERSEDED in the c header).
pub const SUPERSEDED: &str = "the request was replaced by a newer one";

/// what a play request ends with when YouTube Music will not give the song.
pub const UNAVAILABLE: &str = "YouTube Music will not play this track.";

#[derive(Default)]
struct PlaybackTimeline {
    position_ms: u32,
    duration_ms: u32,
    is_playing: bool,
    end_of_track: bool,
    observed_at: Option<Instant>,
}

impl PlaybackTimeline {
    const STALL_TIMEOUT: Duration = Duration::from_secs(6);

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn is_stalled(&self) -> bool {
        self.is_playing
            && self
                .observed_at
                .is_some_and(|observed_at| observed_at.elapsed() >= Self::STALL_TIMEOUT)
    }

    fn observe_position(&mut self, position_ms: u32, is_playing: Option<bool>) {
        self.position_ms = position_ms;
        if let Some(is_playing) = is_playing {
            self.is_playing = is_playing;
        }
        self.observed_at = Some(Instant::now());
    }

    fn current_position_ms(&self) -> u32 {
        let elapsed_ms = if self.is_playing {
            self.observed_at
                .map(|observed_at| {
                    observed_at.elapsed().min(Self::STALL_TIMEOUT).as_millis() as u32
                })
                .unwrap_or(0)
        } else {
            0
        };
        let position_ms = self.position_ms.saturating_add(elapsed_ms);
        if self.duration_ms == 0 {
            position_ms
        } else {
            position_ms.min(self.duration_ms)
        }
    }
}

struct State {
    auth: Option<Auth>,
    api: Option<Arc<Api>>,
    volume: f32,
    stream_quality_kbps: u32,
    volume_normalization: bool,
    audio_cache: Option<(String, u64)>,
    ad_block: bool,
    local_playback: Option<file_playback::LocalPlayback>,
    local_sink: Option<Arc<output::LocalOutput>>,
    live: Option<(Arc<live_input::LiveBuffer>, Arc<AtomicBool>)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            auth: None,
            api: None,
            volume: 0.75,
            stream_quality_kbps: 0,
            volume_normalization: false,
            audio_cache: None,
            ad_block: true,
            local_playback: None,
            local_sink: None,
            live: None,
        }
    }
}

pub struct YtMusicCore {
    state: Mutex<State>,
    playback_generation: watch::Sender<u64>,
    switching: Mutex<()>,
    spectrum: SharedSpectrum,
    dsp: SharedDsp,
    timeline: Arc<Mutex<PlaybackTimeline>>,
    local_playback_active: Arc<AtomicBool>,
    volume_normalization: Arc<AtomicBool>,
    output_health: OutputHealth,
    live_analysis: Mutex<live_input::LiveAnalysis>,
    /// one download per song at a time, shared by preload and play
    downloads: AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl Default for YtMusicCore {
    fn default() -> Self {
        Self::new()
    }
}

fn start_logging() {
    let _ = env_logger::Builder::from_default_env().try_init();
}

fn default_cache_directory() -> PathBuf {
    if let Ok(directory) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(directory).join("tubify/audio");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache/tubify/audio");
    }
    std::env::temp_dir().join("tubify-audio")
}

impl YtMusicCore {
    pub fn new() -> Self {
        start_logging();
        Self {
            state: Mutex::new(State::default()),
            playback_generation: watch::Sender::new(0),
            switching: Mutex::new(()),
            spectrum: SharedSpectrum::new(),
            dsp: SharedDsp::new(),
            timeline: Arc::new(Mutex::new(PlaybackTimeline::default())),
            local_playback_active: Arc::new(AtomicBool::new(false)),
            volume_normalization: Arc::new(AtomicBool::new(false)),
            output_health: OutputHealth::new(),
            live_analysis: Mutex::new(live_input::LiveAnalysis::new()),
            downloads: AsyncMutex::new(HashMap::new()),
        }
    }

    // --- signing in -------------------------------------------------------

    /// signs in with the browser cookie, and completes with the login to keep
    /// (json) so the next launch needs no cookie again.
    pub async fn connect(&self, cookie: String, _client_id: String) -> Result<String, String> {
        let auth = ytm::auth::sign_in(&cookie).await?;
        self.adopt(auth).await
    }

    /// signs in again with the json `connect` returned.
    pub async fn connect_stored(&self, login: String, _client_id: String) -> Result<String, String> {
        let stored = Auth::from_json(&login)?;
        let auth = ytm::auth::sign_in(&stored.cookie).await?;
        self.adopt(auth).await
    }

    async fn adopt(&self, auth: Auth) -> Result<String, String> {
        let ad_block = self.lock_state()?.ad_block;
        let api = Api::new(auth.clone(), ad_block)?;
        let login = auth.to_json();
        {
            let mut state = self.lock_state()?;
            state.auth = Some(auth);
            state.api = Some(Arc::new(api));
        }
        self.output_health.clear();
        Ok(login)
    }

    pub fn disconnect(&self) {
        self.begin_playback_request();
        self.clear_local_playback();
        if let Ok(mut state) = self.lock_state() {
            state.auth = None;
            state.api = None;
            state.local_sink = None;
        }
        self.spectrum.set_enabled(false);
        self.reset_timeline();
    }

    pub fn session_is_valid(&self) -> bool {
        self.lock_state().is_ok_and(|state| state.api.is_some())
    }

    fn api(&self) -> Result<Arc<Api>, String> {
        self.lock_state()?
            .api
            .clone()
            .ok_or_else(|| "Sign in to YouTube Music first.".to_owned())
    }

    /// a library call, as json out. the qt side passes the operation and its
    /// arguments and parses the answer with the readers it already had.
    pub async fn api_request(&self, op: String, args: String) -> Result<String, String> {
        let args: Value = if args.trim().is_empty() {
            Value::Object(serde_json::Map::new())
        } else {
            serde_json::from_str(&args).map_err(|error| format!("bad request arguments: {error}"))?
        };
        let api = self.api()?;
        let answer = api.call(&op, &args).await?;
        serde_json::to_string(&answer).map_err(|error| format!("could not encode the answer: {error}"))
    }

    // --- audio settings ---------------------------------------------------

    pub fn set_preamp_gain(&self, preamp_db: f32) -> Result<(), String> {
        let mut settings = self.dsp.get();
        settings.preamp_db = preamp_db;
        self.dsp.set(settings);
        Ok(())
    }

    pub fn set_equalizer(&self, gains_db: Vec<f32>) -> Result<(), String> {
        if gains_db.len() != EQ_BAND_COUNT {
            return Err(format!(
                "the equalizer takes {EQ_BAND_COUNT} gains, not {}",
                gains_db.len()
            ));
        }
        let mut settings = self.dsp.get();
        for (band, gain) in settings.gains_db.iter_mut().zip(gains_db) {
            *band = gain;
        }
        self.dsp.set(settings);
        Ok(())
    }

    pub fn set_playback_shaping(
        &self,
        tempo: f32,
        pitch_semitones: f32,
        preserves_pitch: bool,
    ) -> Result<(), String> {
        let mut settings = self.dsp.get();
        settings.tempo = tempo;
        settings.pitch_semitones = pitch_semitones;
        settings.preserves_pitch = preserves_pitch;
        self.dsp.set(settings);
        Ok(())
    }

    pub fn set_ad_block(&self, enabled: bool) -> Result<(), String> {
        let api = {
            let mut state = self.lock_state()?;
            state.ad_block = enabled;
            state.api.clone()
        };
        if let Some(api) = api {
            api.set_ad_block(enabled);
        }
        Ok(())
    }

    pub fn ad_block(&self) -> bool {
        self.lock_state().map(|state| state.ad_block).unwrap_or(true)
    }

    pub fn audio_output_stalled(&self) -> bool {
        if self.output_health.is_stalled() {
            return true;
        }
        if self.local_playback_active.load(Ordering::Acquire) {
            return false;
        }
        self.timeline
            .lock()
            .map(|timeline| timeline.is_stalled())
            .unwrap_or(false)
    }

    /// rebuilds the file output, keeping the login.
    pub async fn restart_audio_output(&self) -> Result<(), String> {
        self.begin_playback_request();
        self.output_health.clear();
        self.clear_local_playback();
        stop_cached_local_sink();
        self.lock_state()?.local_sink = None;
        self.reset_timeline();
        Ok(())
    }

    pub fn begin_playback_request(&self) -> u64 {
        let mut ticket = 0;
        self.playback_generation.send_modify(|generation| {
            *generation += 1;
            ticket = *generation;
        });
        ticket
    }

    fn is_latest_request(&self, ticket: u64) -> bool {
        *self.playback_generation.borrow() == ticket
    }

    // --- playing ----------------------------------------------------------

    /// the video id in a `ytm:track:` uri (or a bare id).
    fn video_id(uri: &str) -> Result<String, String> {
        let id = id_from_uri(uri);
        if id.is_empty() {
            return Err("that song has no YouTube Music id".to_owned());
        }
        Ok(id)
    }

    fn cache_settings(&self) -> Result<(PathBuf, u64), String> {
        let state = self.lock_state()?;
        let directory = state
            .audio_cache
            .as_ref()
            .map(|(path, _)| PathBuf::from(path))
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(default_cache_directory);
        let limit = state.audio_cache.as_ref().map(|(_, limit)| *limit).unwrap_or(0);
        Ok((directory, limit))
    }

    async fn download_lock(&self, id: &str) -> Arc<AsyncMutex<()>> {
        let mut downloads = self.downloads.lock().await;
        Arc::clone(
            downloads
                .entry(id.to_owned())
                .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
        )
    }

    async fn fetch_song(&self, id: &str) -> Result<PathBuf, String> {
        let (directory, limit) = self.cache_settings()?;
        if let Some(path) = stream::cached_file(&directory, id) {
            return Ok(path);
        }
        let lock = self.download_lock(id).await;
        let _guard = lock.lock().await;
        if let Some(path) = stream::cached_file(&directory, id) {
            return Ok(path);
        }
        let (quality, cookie) = {
            let state = self.lock_state()?;
            (
                state.stream_quality_kbps,
                state.auth.as_ref().map(|auth| auth.cookie.clone()),
            )
        };
        let path = stream::download(id, &directory, quality, cookie.as_deref()).await?;
        stream::trim_cache(&directory, limit);
        Ok(path)
    }

    pub async fn play_track(&self, uri: String) -> Result<(), String> {
        let ticket = self.begin_playback_request();
        self.play_track_as(uri, ticket).await
    }

    pub async fn play_track_as(&self, uri: String, ticket: u64) -> Result<(), String> {
        if !self.is_latest_request(ticket) {
            return Err(SUPERSEDED.to_owned());
        }
        let id = Self::video_id(&uri)?;
        let path = self.fetch_song(&id).await?;
        if !self.is_latest_request(ticket) {
            return Err(SUPERSEDED.to_owned());
        }
        self.play_path_as(path, ticket)
    }

    /// fetches the next song without starting it, so the handover is quick.
    pub fn preload_track(self: &Arc<Self>, uri: String, _preload_standby: bool) -> Result<(), String> {
        let id = Self::video_id(&uri)?;
        let (directory, limit) = self.cache_settings()?;
        if stream::cached_file(&directory, &id).is_some() {
            return Ok(());
        }
        let (quality, cookie) = {
            let state = self.lock_state()?;
            (
                state.stream_quality_kbps,
                state.auth.as_ref().map(|auth| auth.cookie.clone()),
            )
        };
        let core = Arc::clone(self);
        // this is called from the interface thread, which has no tokio
        // runtime of its own: the warm-up gets a small one on its own thread.
        std::thread::Builder::new()
            .name("tubify-preload".to_owned())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async move {
                    let lock = core.download_lock(&id).await;
                    let _guard = lock.lock().await;
                    if stream::cached_file(&directory, &id).is_some() {
                        return;
                    }
                    if stream::download(&id, &directory, quality, cookie.as_deref()).await.is_ok() {
                        stream::trim_cache(&directory, limit);
                    }
                });
            })
            .ok();
        Ok(())
    }

    pub async fn crossfade_track(&self, uri: String, duration_ms: u32) -> Result<(), String> {
        let ticket = self.begin_playback_request();
        self.crossfade_track_as(uri, duration_ms, ticket).await
    }

    pub async fn crossfade_track_as(
        &self,
        uri: String,
        _duration_ms: u32,
        ticket: u64,
    ) -> Result<(), String> {
        if !self.is_latest_request(ticket) {
            return Err(SUPERSEDED.to_owned());
        }
        // the fetch is the part that takes time; the output switches when it
        // is ready. the crossfade ramp itself lives in the output.
        let id = Self::video_id(&uri)?;
        let path = self.fetch_song(&id).await?;
        if !self.is_latest_request(ticket) {
            return Err(SUPERSEDED.to_owned());
        }
        self.play_path_as(path, ticket)
    }

    pub async fn play_file(&self, path: String) -> Result<(), String> {
        let ticket = self.begin_playback_request();
        self.play_file_as(path, ticket).await
    }

    pub async fn play_file_as(&self, path: String, ticket: u64) -> Result<(), String> {
        if !self.is_latest_request(ticket) {
            return Err(SUPERSEDED.to_owned());
        }
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(format!("{} is no longer there", path.display()));
        }
        self.play_path_as(path, ticket)
    }

    fn open_decoder(
        path: &std::path::Path,
    ) -> Result<rodio::Decoder<io::BufReader<std::fs::File>>, String> {
        let file = std::fs::File::open(path)
            .map_err(|error| format!("failed to open the track: {error}"))?;
        let length = file.metadata().map(|metadata| metadata.len()).ok();
        let mut builder = rodio::Decoder::builder()
            .with_data(io::BufReader::new(file))
            .with_seekable(true);
        if let Some(length) = length {
            builder = builder.with_byte_len(length);
        }
        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            builder = builder.with_hint(extension);
        }
        builder
            .build()
            .map_err(|error| format!("failed to decode the track: {error}"))
    }

    fn play_path_as(&self, path: PathBuf, ticket: u64) -> Result<(), String> {
        let decoder = Self::open_decoder(&path)?;

        let _switching = self.lock_switching();
        if !self.is_latest_request(ticket) {
            return Err(SUPERSEDED.to_owned());
        }
        self.clear_local_playback();

        let duration_ms = decoder
            .total_duration()
            .map(|duration| duration.as_millis() as u64)
            .or_else(|| local::measure_duration_ms(&path))
            .unwrap_or(0);
        let channels = decoder.channels() as u32;
        let sample_rate = decoder.sample_rate();

        let pushed_frames = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let source_frames = Arc::clone(&pushed_frames);
        let spectrum = self.spectrum.clone();
        let replay_gain = local::replay_gain(&path);
        let source = file_playback::LocalTrackSource::new(
            decoder,
            spectrum,
            source_frames,
            Arc::clone(&cancel),
            self.dsp.clone(),
            replay_gain,
            Arc::clone(&self.volume_normalization),
        );

        let sink = self.resolve_live_sink()?;
        sink.stop();
        sink.clear();
        sink.append(source);
        sink.play();

        {
            let mut state = self.lock_state()?;
            state.local_playback = Some(file_playback::LocalPlayback {
                path: path.clone(),
                duration_ms,
                pushed_frames,
                base_position_ms: 0,
                channels,
                sample_rate,
                replay_gain,
                cancel: Arc::clone(&cancel),
            });
        }
        self.local_playback_active.store(true, Ordering::Release);
        self.reset_timeline();
        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.duration_ms = duration_ms.min(u32::MAX as u64) as u32;
            timeline.observe_position(0, Some(true));
        }
        self.spawn_local_end_watcher(cancel);
        Ok(())
    }

    pub fn play(&self) -> Result<(), String> {
        let sink = self.resolve_live_sink()?;
        let position = self.playback_position_ms();
        sink.play();
        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.end_of_track = false;
            timeline.observe_position(position, Some(true));
        }
        Ok(())
    }

    pub fn pause(&self) -> Result<(), String> {
        let position = self.playback_position_ms();
        if let Ok(sink) = self.resolve_live_sink() {
            sink.pause();
        }
        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.observe_position(position, Some(false));
        }
        Ok(())
    }

    pub fn seek(&self, position_ms: u32) -> Result<(), String> {
        if self.local_active()? {
            return self.local_seek(position_ms);
        }
        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.observe_position(position_ms, None);
        }
        Ok(())
    }

    pub fn playback_position_ms(&self) -> u32 {
        if let Ok(state) = self.lock_state()
            && let Some(playback) = &state.local_playback
        {
            return playback.position_ms();
        }
        self.timeline
            .lock()
            .map(|timeline| timeline.current_position_ms())
            .unwrap_or(0)
    }

    pub fn playback_duration_ms(&self) -> u32 {
        if let Ok(state) = self.lock_state()
            && let Some(playback) = &state.local_playback
        {
            return playback.duration_ms.min(u32::MAX as u64) as u32;
        }
        self.timeline
            .lock()
            .map(|timeline| timeline.duration_ms)
            .unwrap_or(0)
    }

    pub fn take_end_of_track(&self) -> bool {
        let Ok(mut timeline) = self.timeline.lock() else {
            return false;
        };
        if !timeline.end_of_track {
            return false;
        }
        timeline.end_of_track = false;
        true
    }

    pub fn set_volume(&self, volume: f32) -> Result<(), String> {
        if !volume.is_finite() {
            return Err("volume must be a finite number".to_owned());
        }
        let volume = volume.clamp(0.0, 1.0);
        let mut state = self.lock_state()?;
        state.volume = volume;
        if let Some(sink) = &state.local_sink {
            sink.set_volume(volume);
        }
        Ok(())
    }

    pub fn volume(&self) -> f32 {
        self.lock_state().map(|state| state.volume).unwrap_or(0.75)
    }

    pub fn set_stream_quality(&self, kbps: u32) -> Result<(), String> {
        self.lock_state()?.stream_quality_kbps = kbps;
        Ok(())
    }

    pub fn set_volume_normalization(&self, enabled: bool) -> Result<(), String> {
        self.lock_state()?.volume_normalization = enabled;
        self.volume_normalization.store(enabled, Ordering::Release);
        Ok(())
    }

    pub fn volume_normalization(&self) -> bool {
        self.lock_state().map(|state| state.volume_normalization).unwrap_or(false)
    }

    pub fn set_audio_cache(&self, path: String, limit_bytes: u64) -> Result<(), String> {
        let mut state = self.lock_state()?;
        state.audio_cache = if path.is_empty() {
            None
        } else {
            Some((path, limit_bytes))
        };
        Ok(())
    }

    // --- live audio -------------------------------------------------------

    pub fn play_live(
        &self,
        buffer: Arc<live_input::LiveBuffer>,
        rate: u32,
        channels: u32,
    ) -> Result<(), String> {
        let sink = self.resolve_live_sink()?;
        {
            let state = self.lock_state()?;
            if state.local_playback.is_none()
                && state.live.as_ref().is_some_and(|(playing, cancel)| {
                    Arc::ptr_eq(playing, &buffer) && !cancel.load(Ordering::Relaxed)
                })
                && !sink.empty()
            {
                drop(state);
                sink.play();
                return Ok(());
            }
        }
        let _switching = self.lock_switching();
        self.begin_playback_request();
        self.clear_local_playback();
        let cancel = Arc::new(AtomicBool::new(false));
        buffer.clear();
        let source = live_input::LiveSource::new(
            Arc::clone(&buffer),
            rate,
            channels,
            Arc::clone(&cancel),
            self.dsp.clone(),
            self.spectrum.clone(),
        );
        sink.stop();
        sink.clear();
        sink.append(source);
        sink.play();
        if let Some((_, previous)) = self.lock_state()?.live.replace((buffer, cancel)) {
            previous.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn output_rate(&self) -> u32 {
        self.resolve_live_sink().map(|sink| sink.rate()).unwrap_or(0)
    }

    pub fn stop_live(&self) {
        let Ok(live) = self.lock_state().map(|mut state| state.live.take()) else {
            return;
        };
        if let Some((_, cancel)) = live {
            cancel.store(true, Ordering::Relaxed);
            if let Ok(sink) = self.resolve_live_sink() {
                sink.stop();
                sink.clear();
            }
            self.spectrum.clear();
        }
    }

    // --- lyrics and the library's fallbacks -------------------------------

    pub async fn lyrics_json(&self, uri: String) -> Result<String, String> {
        let api = self.api()?;
        let lyrics = api.lyrics_json(&uri).await?;
        serde_json::to_string(&lyrics).map_err(|error| format!("could not encode the lyrics: {error}"))
    }

    pub async fn playlist_json(&self, uri: String) -> Result<String, String> {
        let api = self.api()?;
        let page = api.playlist_songs_json(&uri).await?;
        serde_json::to_string(&page).map_err(|error| format!("could not encode the playlist: {error}"))
    }

    pub async fn artist_top_tracks_json(&self, uri: String) -> Result<String, String> {
        let api = self.api()?;
        let page = api.artist_top_tracks_json(&uri).await?;
        serde_json::to_string(&page).map_err(|error| format!("could not encode the songs: {error}"))
    }

    // --- spectrum ---------------------------------------------------------

    pub fn spectrum_levels(&self) -> Vec<f32> {
        self.spectrum.levels()
    }

    pub fn copy_spectrum_levels(&self, out: &mut [f32]) -> usize {
        self.spectrum.copy_levels(out)
    }

    pub fn set_spectrum_enabled(&self, enabled: bool) {
        self.spectrum.set_enabled(enabled);
    }

    pub fn analyse_live(&self, samples: &[f32], output_gain: f64) {
        let mut analysis = self
            .live_analysis
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        analysis.feed(&self.spectrum, samples, output_gain);
    }

    // --- plumbing ---------------------------------------------------------

    fn lock_switching(&self) -> std::sync::MutexGuard<'_, ()> {
        self.switching
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, State>, String> {
        self.state
            .lock()
            .map_err(|_| "the player state is unavailable".to_owned())
    }

    fn local_active(&self) -> Result<bool, String> {
        Ok(self.lock_state()?.local_playback.is_some())
    }

    fn resolve_live_sink(&self) -> Result<Arc<output::LocalOutput>, String> {
        {
            let state = self.lock_state()?;
            if let Some(sink) = &state.local_sink {
                return Ok(Arc::clone(sink));
            }
        }
        let sink = {
            let mut cached = LOCAL_AUDIO_SINK
                .lock()
                .map_err(|_| "the audio output is unavailable".to_owned())?;
            match cached.as_ref() {
                Some(sink) => Arc::clone(sink),
                None => {
                    let created = Arc::new(output::LocalOutput::open().ok_or_else(|| {
                        self.output_health.mark_stalled();
                        "no audio output device is available".to_owned()
                    })?);
                    *cached = Some(Arc::clone(&created));
                    created
                }
            }
        };
        let mut state = self.lock_state()?;
        sink.set_volume(state.volume);
        state.local_sink = Some(Arc::clone(&sink));
        Ok(sink)
    }

    fn clear_local_playback(&self) {
        self.local_playback_active.store(false, Ordering::Release);
        let (local, sink) = match self.lock_state() {
            Ok(mut state) => (state.local_playback.take(), state.local_sink.clone()),
            Err(_) => return,
        };
        if let Some(local) = local {
            local.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(sink) = sink {
            sink.stop();
            sink.clear();
        }
    }

    fn local_seek(&self, position_ms: u32) -> Result<(), String> {
        let playback = self
            .lock_state()?
            .local_playback
            .clone()
            .ok_or_else(|| "nothing is playing".to_owned())?;
        let duration = playback.duration_ms;
        let channels = playback.channels.max(1);
        let sample_rate = playback.sample_rate.max(1);
        let target = (position_ms as u64).min(duration);

        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.end_of_track = false;
        }

        let sink = self.resolve_live_sink()?;
        let was_paused = sink.is_paused();
        playback.cancel.store(true, Ordering::Relaxed);
        sink.clear();
        playback.pushed_frames.store(0, Ordering::Relaxed);

        let path = playback.path.clone();
        let mut decoder = Self::open_decoder(&path)?;
        if decoder.try_seek(Duration::from_millis(target)).is_err() {
            let target_samples = target * sample_rate as u64 / 1_000 * channels as u64;
            let mut skipped: u64 = 0;
            while skipped < target_samples {
                match decoder.next() {
                    Some(_) => skipped += 1,
                    None => break,
                }
            }
        }

        let pushed_frames = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let spectrum = self.spectrum.clone();
        let source_frames = Arc::clone(&pushed_frames);
        let replay_gain = playback.replay_gain;
        let source = file_playback::LocalTrackSource::new(
            decoder,
            spectrum,
            source_frames,
            Arc::clone(&cancel),
            self.dsp.clone(),
            replay_gain,
            Arc::clone(&self.volume_normalization),
        );

        sink.append(source);
        if was_paused {
            sink.pause();
        } else {
            sink.play();
        }

        let mut state = self.lock_state()?;
        state.local_playback = Some(file_playback::LocalPlayback {
            path,
            duration_ms: duration,
            pushed_frames,
            base_position_ms: target,
            channels,
            sample_rate,
            replay_gain,
            cancel: Arc::clone(&cancel),
        });
        drop(state);
        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.end_of_track = false;
            timeline.observe_position(target as u32, Some(!was_paused));
        }
        self.spawn_local_end_watcher(cancel);
        Ok(())
    }

    fn spawn_local_end_watcher(&self, cancel: Arc<AtomicBool>) {
        let sink = match self.resolve_live_sink() {
            Ok(sink) => sink,
            Err(_) => return,
        };
        let timeline = Arc::clone(&self.timeline);
        std::thread::Builder::new()
            .name("tubify-end".to_owned())
            .spawn(move || {
                while !cancel.load(Ordering::Relaxed) && !sink.empty() {
                    std::thread::sleep(Duration::from_millis(200));
                }
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if let Ok(mut timeline) = timeline.lock() {
                    timeline.end_of_track = true;
                    timeline.is_playing = false;
                    timeline.observed_at = Some(Instant::now());
                }
                sink.pause_if_ended();
            })
            .ok();
    }

    fn reset_timeline(&self) {
        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.reset();
        }
    }
}

/// the cached songs on disk, for the settings page.
pub fn cache_usage(directory: &str) -> u64 {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_core_is_signed_out() {
        let core = YtMusicCore::new();
        assert!(!core.session_is_valid());
        assert!(!core.take_end_of_track());
    }

    #[test]
    fn volume_is_remembered_before_signing_in() {
        let core = YtMusicCore::new();
        core.set_volume(0.42).unwrap();
        assert!((core.volume() - 0.42).abs() < 1e-6);
    }

    #[test]
    fn the_equalizer_takes_exactly_its_bands() {
        let core = YtMusicCore::new();
        assert!(core.set_equalizer(vec![0.0; EQ_BAND_COUNT]).is_ok());
        assert!(core.set_equalizer(vec![0.0; 3]).is_err());
    }

    #[test]
    fn a_song_uri_gives_its_video_id() {
        assert_eq!(YtMusicCore::video_id("ytm:track:abc").unwrap(), "abc");
        assert_eq!(YtMusicCore::video_id("abc").unwrap(), "abc");
    }
}
