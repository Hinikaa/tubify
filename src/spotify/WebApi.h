/*
 * Tubify, a YouTube Music player for desktop
 * Copyright (C) 2026  Hinikaa
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

#pragma once

#include "spotify/SpotifyTypes.h"

#include <QByteArray>
#include <QHash>
#include <QJsonDocument>
#include <QJsonObject>
#include <QCoroTask>

namespace swiftify {

class PlaybackCore;

/// the library calls the views need, on top of the core's YouTube Music
/// client. the core answers in the shapes the readers below already knew, so
/// swapping the service did not mean rewriting every view.
class WebApi {
public:
    static constexpr int kMaximumCoverBytes = 256'000;

    explicit WebApi(PlaybackCore *core);

    QCoro::Task<Profile> profile(QString accessToken);
    QCoro::Task<QList<Playlist>> playlists(QString accessToken);
    QCoro::Task<QList<Song>> playlistSongs(QString playlistId, QString accessToken);
    QCoro::Task<QList<Song>> savedSongs(QString accessToken);
    QCoro::Task<int> savedSongTotal(QString accessToken);
    QCoro::Task<QList<Song>> recentlyPlayed(QString accessToken);
    QCoro::Task<SearchResults> search(QString query, QString accessToken);
    QCoro::Task<Song> song(QString trackId, QString accessToken);
    QCoro::Task<Album> album(QString albumId, QString accessToken);
    QCoro::Task<Playlist> playlist(QString playlistId, QString accessToken);
    QCoro::Task<Artist> artist(QString artistId, QString accessToken);
    QCoro::Task<QList<Song>> albumSongs(Album album, QString accessToken);
    QCoro::Task<QList<Album>> artistAlbums(Artist artist, QString accessToken);
    QCoro::Task<QHash<QString, QStringList>> artistGenres(QStringList artistIds, QString accessToken,
                                                         QHash<QString, QUrl> *artwork = nullptr);

    QCoro::Task<Playlist> createPlaylist(QString name, QString description, bool isPublic, QString accessToken);
    QCoro::Task<> updatePlaylist(QString id, QString name, QString description, bool isPublic, QString accessToken);
    QCoro::Task<> addToPlaylist(QString songUri, QString playlistId, QString accessToken);
    QCoro::Task<> removeFromPlaylist(QString songUri, QString playlistId, QString accessToken);
    QCoro::Task<> reorderPlaylist(QString playlistId, int rangeStart, int insertBefore, int length,
                                  QString accessToken);
    QCoro::Task<> setPlaylistImage(QString playlistId, QByteArray jpeg, QString accessToken);
    QCoro::Task<> deletePlaylist(QString id, QString accessToken);
    QCoro::Task<> setSaved(bool saved, QString uri, QString accessToken);
    QCoro::Task<QHash<QString, bool>> savedStatuses(QStringList uris, QString accessToken);

private:
    QCoro::Task<QJsonDocument> call(const QString &op, const QJsonObject &args = {});

    PlaybackCore *m_core;
};

} // namespace swiftify
