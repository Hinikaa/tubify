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

//! a plain c abi over `YtMusicCore` for the linux qt front end. the header is
//! `include/tubify_core.h`.
//!
//! swift-bridge ran the async calls on a runtime of its own. here the core owns
//! one, and an async call returns at once: its result is queued as a completion
//! and a byte is written to a socket whose other end the front end watches, so
//! results are collected on the caller's own thread and nothing calls back into
//! c++ from a tokio worker.
//!
//! strings crossing the boundary are nul-terminated utf-8. any string this file
//! hands out is owned by the caller and released with `tubify_string_free`.
//! a null error string means success.

#[cfg(unix)]
use std::{
    io::Write,
    os::{fd::AsRawFd, unix::net::UnixStream},
};
use std::{
    collections::VecDeque,
    ffi::{CStr, CString, c_char, c_int, c_void},
    future::Future,
    ptr,
    sync::{Arc, Mutex, OnceLock},
};

use tokio::runtime::{Builder, Runtime};

use crate::{YtMusicCore, equalizer_frequencies};

/// two workers are plenty for one session's network traffic, and keep the
/// process from starting a thread per core for a music player.
const RUNTIME_WORKER_THREADS: usize = 2;

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(RUNTIME_WORKER_THREADS)
            .thread_name("tubify-core")
            .enable_all()
            .build()
            .expect("failed to start the Tubify core runtime")
    })
}

struct Completion {
    request_id: u64,
    ok: bool,
    payload: CString,
}

/// what the front end asked to be called with when a completion is queued,
/// from whichever thread queued it (`tubify_core_set_completion_callback`)
struct Waker {
    callback: extern "C" fn(*mut c_void),
    context: *mut c_void,
}

// SAFETY: the front end hands over a callback that may be called from any
// thread, with a context it keeps alive until it takes the callback back.
unsafe impl Send for Waker {}

struct CompletionQueue {
    pending: Mutex<VecDeque<Completion>>,
    /// written to whenever a completion is queued
    #[cfg(unix)]
    notifier: UnixStream,
    /// the end the front end polls. kept here so the descriptor lives exactly
    /// as long as anything can still queue a completion.
    #[cfg(unix)]
    receiver: UnixStream,
    /// called too, where there is no descriptor to poll (windows)
    waker: Mutex<Option<Waker>>,
}

impl CompletionQueue {
    fn new() -> std::io::Result<Self> {
        #[cfg(unix)]
        let (notifier, receiver) = {
            let (notifier, receiver) = UnixStream::pair()?;
            notifier.set_nonblocking(true)?;
            receiver.set_nonblocking(true)?;
            (notifier, receiver)
        };
        Ok(Self {
            pending: Mutex::new(VecDeque::new()),
            #[cfg(unix)]
            notifier,
            #[cfg(unix)]
            receiver,
            waker: Mutex::new(None),
        })
    }

    fn push(&self, request_id: u64, result: Result<String, String>) {
        let (ok, payload) = match result {
            Ok(value) => (true, value),
            Err(error) => (false, error),
        };
        let completion = Completion {
            request_id,
            ok,
            payload: c_string_lossy(payload),
        };
        if let Ok(mut pending) = self.pending.lock() {
            pending.push_back(completion);
        }
        // a full socket already has unread bytes waiting, which wakes the
        // reader just the same, so a would-block write needs no retry.
        #[cfg(unix)]
        let _ = (&self.notifier).write(&[1]);
        // held while calling, so taking the callback back waits for a call
        // under way
        if let Ok(waker) = self.waker.lock()
            && let Some(waker) = waker.as_ref()
        {
            (waker.callback)(waker.context);
        }
    }

    fn pop(&self) -> Option<Completion> {
        self.pending.lock().ok()?.pop_front()
    }
}

/// the handle the front end holds.
pub struct TubifyCore {
    core: Arc<YtMusicCore>,
    completions: Arc<CompletionQueue>,
}

impl TubifyCore {
    fn spawn<F, T>(&self, request_id: u64, operation: impl FnOnce(Arc<YtMusicCore>) -> F)
    where
        F: Future<Output = Result<T, String>> + Send + 'static,
        T: Into<String>,
    {
        let completions = Arc::clone(&self.completions);
        let future = operation(Arc::clone(&self.core));
        runtime().spawn(async move {
            completions.push(request_id, future.await.map(Into::into));
        });
    }
}

struct Unit;

impl From<Unit> for String {
    fn from(_: Unit) -> Self {
        String::new()
    }
}

fn c_string_lossy(value: String) -> CString {
    CString::new(value).unwrap_or_else(|error| {
        let mut bytes = error.into_vec();
        bytes.retain(|byte| *byte != 0);
        CString::new(bytes).unwrap_or_default()
    })
}

fn into_raw(value: String) -> *mut c_char {
    c_string_lossy(value).into_raw()
}

fn error_string(result: Result<(), String>) -> *mut c_char {
    match result {
        Ok(()) => ptr::null_mut(),
        Err(error) => into_raw(error),
    }
}

/// # Safety
/// `value` is null or a nul-terminated string valid for this call.
unsafe fn read_str(value: *const c_char) -> Result<String, String> {
    if value.is_null() {
        return Err("a required string was missing".to_owned());
    }
    // SAFETY: the caller guarantees a valid nul-terminated string.
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| "a string was not valid utf-8".to_owned())
}

/// # Safety
/// `handle` is null or a live pointer from `tubify_core_new`.
unsafe fn handle<'a>(handle: *const TubifyCore) -> Option<&'a TubifyCore> {
    // SAFETY: the caller guarantees the pointer is null or live.
    unsafe { handle.as_ref() }
}

const MISSING_CORE: &str = "the Tubify core is not available";

/// a new core, or null if its completion socket could not be created.
#[unsafe(no_mangle)]
pub extern "C" fn tubify_core_new() -> *mut TubifyCore {
    let Ok(completions) = CompletionQueue::new() else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(TubifyCore {
        core: Arc::new(YtMusicCore::new()),
        completions: Arc::new(completions),
    }))
}

/// # Safety
/// `core` is null or a pointer from `tubify_core_new` that is not used again.
/// calls still running finish in the background and their results are dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_free(core: *mut TubifyCore) {
    if core.is_null() {
        return;
    }
    // SAFETY: the caller hands back ownership of a pointer from `tubify_core_new`.
    let core = unsafe { Box::from_raw(core) };
    core.core.disconnect();
}

/// the descriptor that becomes readable when a completion is waiting. the
/// caller drains it and then calls `tubify_core_take_completion` until it
/// returns false. it stays open until `tubify_core_free`.
///
/// # Safety
/// `core` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_completion_fd(core: *const TubifyCore) -> c_int {
    #[cfg(unix)]
    {
        // SAFETY: forwarded from the caller.
        unsafe { handle(core) }
            .map(|core| core.completions.receiver.as_raw_fd())
            .unwrap_or(-1)
    }
    // no descriptor to poll here: tubify_core_set_completion_callback
    #[cfg(not(unix))]
    {
        let _ = core;
        -1
    }
}

/// `callback(context)` is called, from whichever thread finished the call,
/// each time a completion is queued; null takes it back, waiting for a call
/// under way to return. for where there is no descriptor to poll (windows).
///
/// # Safety
/// `core` is null or live; `context` stays valid until the callback is taken
/// back or `core` is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_set_completion_callback(
    core: *const TubifyCore,
    callback: Option<extern "C" fn(*mut c_void)>,
    context: *mut c_void,
) {
    // SAFETY: forwarded from the caller.
    let Some(core) = (unsafe { handle(core) }) else {
        return;
    };
    if let Ok(mut waker) = core.completions.waker.lock() {
        *waker = callback.map(|callback| Waker { callback, context });
    }
}

/// takes the oldest finished async call. `payload` receives the call's value on
/// success (lyrics json, or an empty string) or its error message, and must be
/// freed with `tubify_string_free`.
///
/// # Safety
/// `core` is null or live; the out pointers are valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_take_completion(
    core: *const TubifyCore,
    request_id: *mut u64,
    ok: *mut bool,
    payload: *mut *mut c_char,
) -> bool {
    // SAFETY: forwarded from the caller.
    let Some(core) = (unsafe { handle(core) }) else {
        return false;
    };
    if request_id.is_null() || ok.is_null() || payload.is_null() {
        return false;
    }
    let Some(completion) = core.completions.pop() else {
        return false;
    };
    // SAFETY: the caller guarantees the out pointers are writable.
    unsafe {
        *request_id = completion.request_id;
        *ok = completion.ok;
        *payload = completion.payload.into_raw();
    }
    true
}

/// # Safety
/// `value` is null or a string returned by this library, not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_string_free(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: every string handed out came from `CString::into_raw`.
        drop(unsafe { CString::from_raw(value) });
    }
}

macro_rules! async_call {
    // `prepare` runs at the call, on the caller's thread, before the work is
    // handed to the runtime: what has to happen in the order calls are made
    ($(#[$doc:meta])* $name:ident($($arg:ident),*) => |$core:ident| prepare { $($prepare:tt)* } run $body:expr) => {
        $(#[$doc])*
        ///
        /// # Safety
        /// `core` is null or live; string arguments are null or valid.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            core: *const TubifyCore,
            request_id: u64,
            $($arg: *const c_char),*
        ) {
            // SAFETY: forwarded from the caller.
            let Some(handle) = (unsafe { handle(core) }) else {
                return;
            };
            $(
                // SAFETY: forwarded from the caller.
                let $arg = match unsafe { read_str($arg) } {
                    Ok(value) => value,
                    Err(error) => {
                        handle.completions.push(request_id, Err(error));
                        return;
                    }
                };
            )*
            #[allow(unused_variables)]
            let $core = &handle.core;
            $($prepare)*
            handle.spawn(request_id, move |$core| async move { $body });
        }
    };
    ($(#[$doc:meta])* $name:ident($($arg:ident),*) => |$core:ident| $body:expr) => {
        async_call!($(#[$doc])* $name($($arg),*) => |$core| prepare {} run $body);
    };
}

async_call!(
    /// signs in with the browser cookie from music.youtube.com and builds the
    /// library client. completes with the login to reconnect with
    /// (tubify_core_connect_stored).
    tubify_core_connect(cookie, client_id) => |core| {
        core.connect(cookie, client_id).await
    }
);
async_call!(
    /// signs in by reading the cookie from a browser's own profile.
    tubify_core_connect_browser(browser) => |core| {
        core.connect_browser(browser).await
    }
);
async_call!(
    /// one library call. `op` names the operation and `args` is its json
    /// object; the call completes with the answer as json.
    tubify_core_api_request(op, args) => |core| {
        core.api_request(op, args).await
    }
);
async_call!(
    /// opens a session with a login an earlier connection completed with: no
    /// token is spent. completes with the login to keep from then on.
    tubify_core_connect_stored(login, client_id) => |core| {
        core.connect_stored(login, client_id).await
    }
);
async_call!(
    /// loads a track and waits until it is playing, unavailable or stopped.
    /// a play call made after it replaces it: it then completes with an
    /// error and leaves the newer call's song alone.
    tubify_core_play_track(media_uri) => |core| prepare {
        let ticket = core.begin_playback_request();
    } run {
        core.play_track_as(media_uri, ticket).await.map(|()| Unit)
    }
);
async_call!(
    /// rebuilds the audio output and keeps the session.
    tubify_core_restart_audio_output() => |core| {
        core.restart_audio_output().await.map(|()| Unit)
    }
);

/// starts a preloaded track and overlaps it with the one already playing.
///
/// # Safety
/// `core` is null or live; `media_uri` is a nul-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_crossfade_track(
    core: *const TubifyCore,
    request_id: u64,
    media_uri: *const c_char,
    duration_ms: u32,
) {
    // SAFETY: forwarded from the caller.
    let Some(handle) = (unsafe { handle(core) }) else {
        return;
    };
    // SAFETY: forwarded from the caller.
    let media_uri = match unsafe { read_str(media_uri) } {
        Ok(uri) => uri,
        Err(error) => {
            handle.completions.push(request_id, Err(error));
            return;
        }
    };
    let ticket = handle.core.begin_playback_request();
    handle.spawn(request_id, move |core| async move {
        core.crossfade_track_as(media_uri, duration_ms, ticket)
            .await
            .map(|()| Unit)
    });
}

/// prepares the active player, and optionally the standby player, without starting a track.
///
/// # Safety
/// `core` is null or live; `media_uri` is a nul-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_preload_track(
    core: *const TubifyCore,
    media_uri: *const c_char,
    preload_standby: c_int,
) -> *mut c_char {
    // SAFETY: forwarded from the caller.
    let Some(handle) = (unsafe { handle(core) }) else {
        return into_raw(MISSING_CORE.to_owned());
    };
    // SAFETY: forwarded from the caller.
    match unsafe { read_str(media_uri) }
        .and_then(|uri| handle.core.preload_track(uri, preload_standby != 0))
    {
        Ok(()) => ptr::null_mut(),
        Err(error) => into_raw(error),
    }
}
async_call!(
    /// plays a music file from anywhere on disk; replaced by a later play
    /// call, as above
    tubify_core_play_file(path) => |core| prepare {
        let ticket = core.begin_playback_request();
    } run {
        core.play_file_as(path, ticket).await.map(|()| Unit)
    }
);
async_call!(
    /// completes with the lyrics as json.
    tubify_core_lyrics_json(media_uri) => |core| {
        core.lyrics_json(media_uri).await
    }
);
async_call!(
    /// completes with a playlist's songs as json, read through the session.
    tubify_core_playlist_json(playlist_uri) => |core| {
        core.playlist_json(playlist_uri).await
    }
);
async_call!(
    /// completes with an artist's top songs as json, read through the session.
    tubify_core_artist_top_tracks_json(artist_uri) => |core| {
        core.artist_top_tracks_json(artist_uri).await
    }
);

/// what a music file's tags say, as json (crate::local::LocalTags), or null
/// with `*error` set. an embedded cover is written into `artwork_dir`. it
/// needs no core and may be called from any thread.
///
/// # Safety
/// `path` and `artwork_dir` are null or valid strings; `error` is null or
/// valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_read_local_tags(
    path: *const c_char,
    artwork_dir: *const c_char,
    error: *mut *mut c_char,
) -> *mut c_char {
    let fail = |message: String| {
        if !error.is_null() {
            // SAFETY: the caller guarantees `error` is writable.
            unsafe { *error = into_raw(message) };
        }
        ptr::null_mut()
    };
    // SAFETY: forwarded from the caller.
    let path = match unsafe { read_str(path) } {
        Ok(path) => path,
        Err(message) => return fail(message),
    };
    // SAFETY: forwarded from the caller.
    let artwork_dir = (!artwork_dir.is_null())
        .then(|| unsafe { read_str(artwork_dir) })
        .transpose();
    let artwork_dir = match artwork_dir {
        Ok(directory) => directory.map(std::path::PathBuf::from),
        Err(message) => return fail(message),
    };
    match crate::local::read_tags(std::path::Path::new(&path), artwork_dir.as_deref()) {
        Ok(tags) => match serde_json::to_string(&tags) {
            Ok(json) => into_raw(json),
            Err(message) => fail(message.to_string()),
        },
        Err(message) => fail(message),
    }
}

/// writes edits (crate::local::TagEdits, as json) into a music file's tags,
/// with `artwork` (`artwork_len` bytes of an image, or null) as its front
/// cover. returns null on success, or an error message to free. the file is
/// left as it was if anything fails. needs no core and may be called from
/// any thread.
///
/// # Safety
/// `path` and `edits` are null or valid strings; `artwork` is null or valid
/// for `artwork_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_write_local_tags(
    path: *const c_char,
    edits: *const c_char,
    artwork: *const u8,
    artwork_len: usize,
) -> *mut c_char {
    // SAFETY: forwarded from the caller.
    let (path, edits) = match unsafe { (read_str(path), read_str(edits)) } {
        (Ok(path), Ok(edits)) => (path, edits),
        (Err(message), _) | (_, Err(message)) => return into_raw(message),
    };
    let edits: crate::local::TagEdits = match serde_json::from_str(&edits) {
        Ok(edits) => edits,
        Err(message) => return into_raw(format!("the edits could not be read: {message}")),
    };
    let artwork = (!artwork.is_null() && artwork_len > 0)
        // SAFETY: the caller guarantees `artwork_len` readable bytes.
        .then(|| unsafe { std::slice::from_raw_parts(artwork, artwork_len) });
    error_string(crate::local::write_tags(
        std::path::Path::new(&path),
        &edits,
        artwork,
    ))
}

/// # Safety
/// `core` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_disconnect(core: *const TubifyCore) {
    // SAFETY: forwarded from the caller.
    if let Some(core) = unsafe { handle(core) } {
        core.core.disconnect();
    }
}

macro_rules! fallible_call {
    ($name:ident($($arg:ident: $ty:ty),*) => |$core:ident| $body:expr) => {
        /// returns null on success, or an error message to free.
        ///
        /// # Safety
        /// `core` is null or live.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(core: *const TubifyCore $(, $arg: $ty)*) -> *mut c_char {
            // SAFETY: forwarded from the caller.
            match unsafe { handle(core) } {
                Some(handle) => {
                    let $core = &handle.core;
                    error_string($body)
                }
                None => into_raw(MISSING_CORE.to_owned()),
            }
        }
    };
}

fallible_call!(tubify_core_play() => |core| core.play());
fallible_call!(tubify_core_pause() => |core| core.pause());
fallible_call!(tubify_core_seek(position_ms: u32) => |core| core.seek(position_ms));
fallible_call!(tubify_core_set_volume(volume: f32) => |core| core.set_volume(volume));
fallible_call!(tubify_core_set_stream_quality(kbps: u32) => |core| core.set_stream_quality(kbps));
fallible_call!(tubify_core_set_volume_normalization(enabled: bool) => |core| core.set_volume_normalization(enabled));
fallible_call!(tubify_core_set_network_filter(enabled: bool) => |core| core.set_network_filter(enabled));
/// keeps streamed songs in `path`, up to `limit_bytes`; an empty path keeps
/// none. applies the next time playback connects. returns null on success, or
/// an error message to free.
///
/// # Safety
/// `core` is null or live; `path` is a nul-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_set_audio_cache(
    core: *const TubifyCore,
    path: *const c_char,
    limit_bytes: u64,
) -> *mut c_char {
    // SAFETY: forwarded from the caller.
    let Some(handle) = (unsafe { handle(core) }) else {
        return into_raw(MISSING_CORE.to_owned());
    };
    // SAFETY: forwarded from the caller.
    match unsafe { read_str(path) } {
        Ok(path) => error_string(handle.core.set_audio_cache(path, limit_bytes)),
        Err(error) => into_raw(error),
    }
}
fallible_call!(tubify_core_set_playback_shaping(tempo: f32, pitch_semitones: f32, preserves_pitch: bool)
    => |core| core.set_playback_shaping(tempo, pitch_semitones, preserves_pitch));
fallible_call!(tubify_core_set_preamp_gain(gain_db: f32) => |core| core.set_preamp_gain(gain_db));

/// returns null on success, or an error message to free.
///
/// # Safety
/// `core` is null or live; `gains_db` points to `count` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_set_equalizer(
    core: *const TubifyCore,
    gains_db: *const f32,
    count: usize,
) -> *mut c_char {
    // SAFETY: forwarded from the caller.
    let Some(core) = (unsafe { handle(core) }) else {
        return into_raw(MISSING_CORE.to_owned());
    };
    if gains_db.is_null() {
        return into_raw("equalizer gains were missing".to_owned());
    }
    // SAFETY: the caller guarantees `count` readable floats.
    let gains = unsafe { std::slice::from_raw_parts(gains_db, count) }.to_vec();
    error_string(core.core.set_equalizer(gains))
}

macro_rules! query {
    ($name:ident -> $ty:ty, $fallback:expr, |$core:ident| $body:expr) => {
        /// # Safety
        /// `core` is null or live.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(core: *const TubifyCore) -> $ty {
            // SAFETY: forwarded from the caller.
            match unsafe { handle(core) } {
                Some(handle) => {
                    let $core = &handle.core;
                    $body
                }
                None => $fallback,
            }
        }
    };
}

query!(tubify_core_session_is_valid -> bool, false, |core| core.session_is_valid());
query!(tubify_core_audio_output_stalled -> bool, false, |core| core.audio_output_stalled());
query!(tubify_core_playback_position_ms -> u32, 0, |core| core.playback_position_ms());
query!(tubify_core_playback_duration_ms -> u32, 0, |core| core.playback_duration_ms());
query!(tubify_core_playback_sample_rate -> u32, 0, |core| core.playback_format().0);
query!(tubify_core_playback_bitrate_kbps -> u32, 0, |core| core.playback_format().1);
query!(tubify_core_playback_bit_depth -> u32, 0, |core| core.playback_format().2);
query!(tubify_core_take_end_of_track -> bool, false, |core| core.take_end_of_track());
query!(tubify_core_volume -> f32, 0.75, |core| core.volume());
query!(tubify_core_output_rate -> u32, 0, |core| core.output_rate());
query!(tubify_core_network_filter -> bool, true, |core| core.network_filter());

/// # Safety
/// `core` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_set_spectrum_enabled(
    core: *const TubifyCore,
    enabled: bool,
) {
    // SAFETY: forwarded from the caller.
    if let Some(core) = unsafe { handle(core) } {
        core.core.set_spectrum_enabled(enabled);
    }
}

/// copies up to `capacity` band levels (0...1) into `out` and returns how many
/// were written. allocates nothing, so it is safe to call every frame.
///
/// # Safety
/// `core` is null or live; `out` points to `capacity` writable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_spectrum_levels(
    core: *const TubifyCore,
    out: *mut f32,
    capacity: usize,
) -> usize {
    // SAFETY: forwarded from the caller.
    let Some(core) = (unsafe { handle(core) }) else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees `capacity` writable floats.
    let out = unsafe { std::slice::from_raw_parts_mut(out, capacity) };
    core.core.copy_spectrum_levels(out)
}

/// the equalizer's centre frequencies in hz, in band order. returns how many
/// were written.
///
/// # Safety
/// `out` points to `capacity` writable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_equalizer_frequencies(out: *mut f32, capacity: usize) -> usize {
    if out.is_null() {
        return 0;
    }
    let frequencies = equalizer_frequencies();
    let count = frequencies.len().min(capacity);
    // SAFETY: the caller guarantees `capacity` writable floats.
    unsafe { std::slice::from_raw_parts_mut(out, count) }.copy_from_slice(&frequencies[..count]);
    count
}

#[cfg(test)]
mod tests {
    use std::{io::Read, time::Duration};

    use super::*;

    fn wait_for_completion(core: *const TubifyCore) -> (u64, bool, String) {
        let fd = unsafe { tubify_core_completion_fd(core) };
        assert!(fd >= 0);
        let reader = unsafe { &(&*core).completions.receiver };
        for _ in 0..200 {
            let mut byte = [0; 16];
            let _ = (&*reader).read(&mut byte);
            let mut request_id = 0;
            let mut ok = true;
            let mut payload = ptr::null_mut();
            if unsafe {
                tubify_core_take_completion(core, &mut request_id, &mut ok, &mut payload)
            } {
                let text = unsafe { CStr::from_ptr(payload) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { tubify_string_free(payload) };
                return (request_id, ok, text);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("no completion arrived");
    }

    #[test]
    fn an_async_call_without_a_session_completes_with_its_error() {
        let core = tubify_core_new();
        let uri = CString::new("spotify:track:4uLU6hMCjMI75M1A2tKUQC").unwrap();
        unsafe { tubify_core_play_track(core, 7, uri.as_ptr()) };

        let (request_id, ok, payload) = wait_for_completion(core);
        assert_eq!(request_id, 7);
        assert!(!ok);
        assert!(!payload.is_empty());
        unsafe { tubify_core_free(core) };
    }

    #[test]
    fn a_missing_argument_completes_instead_of_crashing() {
        let core = tubify_core_new();
        unsafe { tubify_core_lyrics_json(core, 3, ptr::null()) };

        let (request_id, ok, _) = wait_for_completion(core);
        assert_eq!(request_id, 3);
        assert!(!ok);
        unsafe { tubify_core_free(core) };
    }

    #[test]
    fn sync_calls_report_errors_and_values() {
        let core = tubify_core_new();
        let error = unsafe { tubify_core_play(core) };
        assert!(!error.is_null());
        unsafe { tubify_string_free(error) };

        assert!(unsafe { tubify_core_set_volume(core, 0.4) }.is_null());
        assert!((unsafe { tubify_core_volume(core) } - 0.4).abs() < f32::EPSILON);

        let mut levels = [1.0; 64];
        let count =
            unsafe { tubify_core_spectrum_levels(core, levels.as_mut_ptr(), levels.len()) };
        assert_eq!(count, 48);
        assert!(levels[..count].iter().all(|level| *level == 0.0));

        let mut frequencies = [0.0; 8];
        let count =
            unsafe { tubify_equalizer_frequencies(frequencies.as_mut_ptr(), frequencies.len()) };
        assert_eq!(
            &frequencies[..count],
            &[60.0, 150.0, 400.0, 1000.0, 2400.0, 15000.0]
        );
        unsafe { tubify_core_free(core) };
    }

    #[test]
    fn the_header_names_the_superseded_message() {
        // the front end tells a replaced request from a failed one by it
        let header = include_str!("../include/tubify_core.h");
        let define = format!("#define SWIFTIFY_CORE_SUPERSEDED \"{}\"", crate::SUPERSEDED);
        assert!(header.contains(&define));
    }

    #[test]
    fn the_header_names_the_unavailable_message() {
        // the front end skips a track spotify will not play by it
        let header = include_str!("../include/tubify_core.h");
        let define = format!("#define SWIFTIFY_CORE_UNAVAILABLE \"{}\"", crate::UNAVAILABLE);
        assert!(header.contains(&define));
    }

    #[test]
    fn null_handles_are_harmless() {
        assert_eq!(unsafe { tubify_core_completion_fd(ptr::null()) }, -1);
        assert!(!unsafe { tubify_core_session_is_valid(ptr::null()) });
        let error = unsafe { tubify_core_pause(ptr::null()) };
        assert!(!error.is_null());
        unsafe { tubify_string_free(error) };
        unsafe { tubify_core_free(ptr::null_mut()) };
    }
}

/// audio another program plays, handed over as it plays (live_input.rs)
pub struct TubifyLiveBuffer {
    buffer: Arc<crate::live_input::LiveBuffer>,
    rate: u32,
    channels: u32,
}

/// a buffer for interleaved f32 audio at `rate` and `channels`. it outlives
/// any core it plays through; free it with `tubify_live_buffer_free`.
#[unsafe(no_mangle)]
pub extern "C" fn tubify_live_buffer_new(rate: u32, channels: u32) -> *mut TubifyLiveBuffer {
    Box::into_raw(Box::new(TubifyLiveBuffer {
        buffer: Arc::new(crate::live_input::LiveBuffer::new(rate, channels)),
        rate,
        channels,
    }))
}

/// # Safety
/// `buffer` is null or a pointer from `tubify_live_buffer_new` that is not
/// used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_live_buffer_free(buffer: *mut TubifyLiveBuffer) {
    if !buffer.is_null() {
        // SAFETY: the caller hands back ownership.
        drop(unsafe { Box::from_raw(buffer) });
    }
}

/// adds `count` interleaved samples. safe to call from any thread, such as
/// the one recording them.
///
/// # Safety
/// `buffer` is null or live; `samples` points to `count` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_live_buffer_push(
    buffer: *const TubifyLiveBuffer,
    samples: *const f32,
    count: usize,
) {
    // SAFETY: forwarded from the caller.
    let Some(buffer) = (unsafe { buffer.as_ref() }) else {
        return;
    };
    if samples.is_null() || count == 0 {
        return;
    }
    // SAFETY: the caller guarantees `count` readable floats.
    buffer
        .buffer
        .push(unsafe { std::slice::from_raw_parts(samples, count) });
}

/// plays what is pushed into `buffer` through the equalizer and the spectrum,
/// in place of whatever played; one already playing carries on. returns null
/// on success, or an error message to free.
///
/// # Safety
/// `core` and `buffer` are null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_play_live(
    core: *const TubifyCore,
    buffer: *const TubifyLiveBuffer,
) -> *mut c_char {
    // SAFETY: forwarded from the caller.
    let Some(core) = (unsafe { handle(core) }) else {
        return into_raw(MISSING_CORE.to_owned());
    };
    // SAFETY: forwarded from the caller.
    let Some(buffer) = (unsafe { buffer.as_ref() }) else {
        return into_raw("the live buffer was missing".to_owned());
    };
    error_string(core.core.play_live(Arc::clone(&buffer.buffer), buffer.rate, buffer.channels))
}

/// hands `count` interleaved stereo samples, heard at `output_gain` of their
/// full level, to the spectrum alone, for audio another program plays to the
/// speakers itself. safe to call from any thread, such as the one recording.
///
/// # Safety
/// `core` is null or live; `samples` points to `count` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_analyse_live(
    core: *const TubifyCore,
    samples: *const f32,
    count: usize,
    output_gain: f32,
) {
    // SAFETY: forwarded from the caller.
    let Some(core) = (unsafe { handle(core) }) else {
        return;
    };
    if samples.is_null() || count == 0 {
        return;
    }
    // SAFETY: the caller guarantees `count` readable floats.
    core.core
        .analyse_live(unsafe { std::slice::from_raw_parts(samples, count) }, f64::from(output_gain));
}

/// # Safety
/// `core` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tubify_core_stop_live(core: *const TubifyCore) {
    // SAFETY: forwarded from the caller.
    if let Some(core) = unsafe { handle(core) } {
        core.core.stop_live();
    }
}
