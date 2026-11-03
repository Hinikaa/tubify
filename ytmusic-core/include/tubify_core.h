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

/*
 * the c interface to ytmusic-core, built with `--features capi`.
 *
 * strings are nul-terminated utf-8. a string returned from here is owned by the
 * caller and released with tubify_string_free. functions returning an error
 * string return null on success.
 *
 * async calls return immediately. their result is queued as a completion and
 * the descriptor from tubify_core_completion_fd becomes readable: drain it,
 * then call tubify_core_take_completion until it returns false.
 */
#ifndef TUBIFY_CORE_H
#define TUBIFY_CORE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct TubifyCore TubifyCore;

TubifyCore *tubify_core_new(void);
void tubify_core_free(TubifyCore *core);
void tubify_string_free(char *value);

int tubify_core_completion_fd(const TubifyCore *core);
/* where there is no descriptor (windows: it is -1), callback(context) is
 * called from the finishing thread instead each time a completion is queued;
 * null takes it back. context lives until then. */
void tubify_core_set_completion_callback(const TubifyCore *core, void (*callback)(void *),
                                         void *context);
bool tubify_core_take_completion(const TubifyCore *core, uint64_t *request_id, bool *ok,
                                 char **payload);

/* async: each completes with `request_id`. lyrics and library calls complete
 * with json, as does a connect (the login to reconnect with), the rest with an
 * empty payload on success.
 *
 * a play call made while an earlier one is still running replaces it, as do
 * an output restart and a new connection: the earlier one then fails with
 * TUBIFY_CORE_SUPERSEDED, having started no sound. */
#define TUBIFY_CORE_SUPERSEDED "the request was replaced by a newer one"
/* a play call for a track YouTube Music will not give this account fails with this */
#define TUBIFY_CORE_UNAVAILABLE "YouTube Music will not play this track."

/* signs in with the Cookie header copied from a signed-in music.youtube.com.
 * completes with the login (json) to reconnect with. */
void tubify_core_connect(const TubifyCore *core, uint64_t request_id, const char *cookie,
                         const char *client_id);
/* signs in again with the login tubify_core_connect returned. completes with
 * the login to keep from then on. */
void tubify_core_connect_stored(const TubifyCore *core, uint64_t request_id, const char *login,
                                const char *client_id);
/* signs in by reading the cookie out of a browser's own profile. `browser` is
 * firefox, chrome, chromium, brave or helium. completes with the login. */
void tubify_core_connect_browser(const TubifyCore *core, uint64_t request_id,
                                 const char *browser);

/* one library operation. `op` names it (profile, playlists, playlistSongs,
 * savedSongs, recentlyPlayed, search, song, album, albumSongs, artist,
 * artistAlbums, artistTopTracks, setSaved, createPlaylist, ...) and `args` is
 * a json object. completes with the answer as json. */
void tubify_core_api_request(const TubifyCore *core, uint64_t request_id, const char *op,
                             const char *args);

void tubify_core_play_track(const TubifyCore *core, uint64_t request_id, const char *media_uri);
/* fetches the next song without starting it. */
char *tubify_core_preload_track(const TubifyCore *core, const char *media_uri,
                                int preload_standby);
/* starts a song, overlapping it with the current one for duration_ms. */
void tubify_core_crossfade_track(const TubifyCore *core, uint64_t request_id,
                                 const char *media_uri, uint32_t duration_ms);
void tubify_core_restart_audio_output(const TubifyCore *core, uint64_t request_id);
void tubify_core_lyrics_json(const TubifyCore *core, uint64_t request_id, const char *media_uri);
/* a playlist's songs, as json shaped like the web api's
 * /v1/playlists/{id}/items page. */
void tubify_core_playlist_json(const TubifyCore *core, uint64_t request_id,
                               const char *playlist_uri);
/* an artist's top songs, as json shaped like /v1/artists/{id}/top-tracks. */
void tubify_core_artist_top_tracks_json(const TubifyCore *core, uint64_t request_id,
                                        const char *artist_uri);
/* plays a music file of the person's own from anywhere on disk: mp3, flac,
 * ogg vorbis, wav, aiff, or aac or apple lossless in an m4a. */
void tubify_core_play_file(const TubifyCore *core, uint64_t request_id, const char *path);

/* what a music file's tags say, as json: title, artist, album, albumArtist,
 * trackNumber, discNumber, date, genre (each null when the file does not
 * say), durationMs and artworkPath. null with *error set if it cannot be read.
 * needs no core and may be called from any thread. */
char *tubify_read_local_tags(const char *path, const char *artwork_dir, char **error);
/* writes edits into a music file's tags. returns null, or an error to free;
 * the file is left as it was if anything fails. needs no core. */
char *tubify_write_local_tags(const char *path, const char *edits, const uint8_t *artwork,
                              size_t artwork_len);

void tubify_core_disconnect(const TubifyCore *core);

char *tubify_core_play(const TubifyCore *core);
char *tubify_core_pause(const TubifyCore *core);
char *tubify_core_seek(const TubifyCore *core, uint32_t position_ms);
char *tubify_core_set_volume(const TubifyCore *core, float volume);
char *tubify_core_set_stream_quality(const TubifyCore *core, uint32_t kbps);
char *tubify_core_set_volume_normalization(const TubifyCore *core, bool enabled);
/* filters sponsored shelves and ad placements from every library answer */
char *tubify_core_set_network_filter(const TubifyCore *core, bool enabled);
/* keeps fetched songs in path (empty: none), up to limit_bytes */
char *tubify_core_set_audio_cache(const TubifyCore *core, const char *path, uint64_t limit_bytes);
char *tubify_core_set_playback_shaping(const TubifyCore *core, float tempo,
                                       float pitch_semitones, bool preserves_pitch);
char *tubify_core_set_preamp_gain(const TubifyCore *core, float gain_db);
char *tubify_core_set_equalizer(const TubifyCore *core, const float *gains_db, size_t count);

bool tubify_core_session_is_valid(const TubifyCore *core);
bool tubify_core_audio_output_stalled(const TubifyCore *core);
uint32_t tubify_core_playback_position_ms(const TubifyCore *core);
uint32_t tubify_core_playback_duration_ms(const TubifyCore *core);
uint32_t tubify_core_playback_sample_rate(const TubifyCore *core);
uint32_t tubify_core_playback_bitrate_kbps(const TubifyCore *core);
uint32_t tubify_core_playback_bit_depth(const TubifyCore *core);
bool tubify_core_take_end_of_track(const TubifyCore *core);
float tubify_core_volume(const TubifyCore *core);
bool tubify_core_network_filter(const TubifyCore *core);
/* the rate the output for files and live audio runs at (opening it); 0 if none */
uint32_t tubify_core_output_rate(const TubifyCore *core);

void tubify_core_set_spectrum_enabled(const TubifyCore *core, bool enabled);
/* 48 bands in 0...1. allocates nothing; returns how many were written. */
size_t tubify_core_spectrum_levels(const TubifyCore *core, float *out, size_t capacity);

/* the equalizer's centre frequencies in hz; returns how many were written. */
size_t tubify_equalizer_frequencies(float *out, size_t capacity);

/* audio another program plays, handed over as it plays: interleaved f32 at
 * the rate and channels it was made for. it outlives any core it plays
 * through. push may be called from any thread. */
typedef struct TubifyLiveBuffer TubifyLiveBuffer;
TubifyLiveBuffer *tubify_live_buffer_new(uint32_t rate, uint32_t channels);
void tubify_live_buffer_free(TubifyLiveBuffer *buffer);
void tubify_live_buffer_push(const TubifyLiveBuffer *buffer, const float *samples, size_t count);
/* plays the buffer through the equalizer and the spectrum in place of
 * whatever played; one already playing carries on */
char *tubify_core_play_live(const TubifyCore *core, const TubifyLiveBuffer *buffer);
void tubify_core_stop_live(const TubifyCore *core);
/* hands interleaved stereo audio, heard at output_gain of its full level, to
 * the spectrum alone. may be called from any thread. */
void tubify_core_analyse_live(const TubifyCore *core, const float *samples, size_t count,
                              float output_gain);

#ifdef __cplusplus
}
#endif

#endif
