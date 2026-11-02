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

#include "spotify/WebApi.h"

#include "core/PlaybackCore.h"
#include "spotify/SpotifyError.h"
#include "spotify/SpotifyJson.h"

#include <QJsonArray>

namespace swiftify {

namespace {

QList<Song> pageSongs(const QJsonObject &page)
{
    QList<Song> songs;
    for (const QJsonValue &value : page.value(QStringLiteral("items")).toArray()) {
        if (const auto song = json::wrappedTrack(value.toObject())) {
            songs.append(*song);
        }
    }
    return songs;
}

QList<Album> pageAlbums(const QJsonObject &page)
{
    QList<Album> albums;
    for (const QJsonValue &value : page.value(QStringLiteral("items")).toArray()) {
        if (const auto album = json::album(value.toObject())) {
            albums.append(*album);
        }
    }
    return albums;
}

QList<Artist> pageArtists(const QJsonObject &page)
{
    QList<Artist> artists;
    for (const QJsonValue &value : page.value(QStringLiteral("items")).toArray()) {
        if (const auto artist = json::artist(value.toObject())) {
            artists.append(*artist);
        }
    }
    return artists;
}

QList<Playlist> pagePlaylists(const QJsonObject &page)
{
    QList<Playlist> playlists;
    for (const QJsonValue &value : page.value(QStringLiteral("items")).toArray()) {
        const Playlist playlist = json::playlist(value.toObject());
        if (playlist.isValid()) {
            playlists.append(playlist);
        }
    }
    return playlists;
}

QJsonArray uriArray(const QStringList &uris)
{
    QJsonArray array;
    for (const QString &uri : uris) {
        array.append(uri);
    }
    return array;
}

} // namespace

WebApi::WebApi(PlaybackCore *core)
    : m_core(core)
{
}

QCoro::Task<QJsonDocument> WebApi::call(const QString &op, const QJsonObject &args)
{
    const QString answer = co_await m_core->apiRequest(
        op, QString::fromUtf8(QJsonDocument(args).toJson(QJsonDocument::Compact)));
    QJsonParseError error;
    const QJsonDocument document = QJsonDocument::fromJson(answer.toUtf8(), &error);
    if (error.error != QJsonParseError::NoError) {
        throw SpotifyError::invalidData(error.errorString());
    }
    co_return document;
}

QCoro::Task<Profile> WebApi::profile(QString accessToken)
{
    Q_UNUSED(accessToken)
    const Profile result = json::profile((co_await call(QStringLiteral("profile"))).object());
    if (!result.isValid()) {
        throw SpotifyError::invalidData(QStringLiteral("the profile had no id."));
    }
    co_return result;
}

QCoro::Task<QList<Playlist>> WebApi::playlists(QString accessToken)
{
    Q_UNUSED(accessToken)
    co_return pagePlaylists((co_await call(QStringLiteral("playlists"))).object());
}

QCoro::Task<QList<Song>> WebApi::playlistSongs(QString playlistId, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_return pageSongs((co_await call(QStringLiteral("playlistSongs"),
                                       {{QStringLiteral("id"), playlistId}})).object());
}

QCoro::Task<QList<Song>> WebApi::savedSongs(QString accessToken)
{
    Q_UNUSED(accessToken)
    co_return pageSongs((co_await call(QStringLiteral("savedSongs"))).object());
}

QCoro::Task<int> WebApi::savedSongTotal(QString accessToken)
{
    Q_UNUSED(accessToken)
    co_return (co_await call(QStringLiteral("savedSongTotal"))).object()
        .value(QStringLiteral("total")).toInt();
}

QCoro::Task<QList<Song>> WebApi::recentlyPlayed(QString accessToken)
{
    Q_UNUSED(accessToken)
    co_return pageSongs((co_await call(QStringLiteral("recentlyPlayed"))).object());
}

QCoro::Task<SearchResults> WebApi::search(QString query, QString accessToken)
{
    Q_UNUSED(accessToken)
    const QJsonObject response = (co_await call(QStringLiteral("search"),
                                                {{QStringLiteral("query"), query}})).object();
    const auto items = [&response](const char *key) {
        return response.value(QLatin1StringView(key)).toObject().value(QStringLiteral("items")).toArray();
    };

    SearchResults results;
    for (const QJsonValue &value : items("tracks")) {
        if (const auto song = json::track(value.toObject())) {
            results.songs.append(*song);
        }
    }
    for (const QJsonValue &value : items("albums")) {
        if (const auto album = json::album(value.toObject())) {
            results.albums.append(*album);
        }
    }
    for (const QJsonValue &value : items("artists")) {
        if (const auto artist = json::artist(value.toObject())) {
            results.artists.append(*artist);
        }
    }
    for (const QJsonValue &value : items("playlists")) {
        const Playlist playlist = json::playlist(value.toObject());
        if (playlist.isValid()) {
            results.playlists.append(playlist);
        }
    }
    co_return results;
}

QCoro::Task<Song> WebApi::song(QString trackId, QString accessToken)
{
    Q_UNUSED(accessToken)
    const auto song = json::track((co_await call(QStringLiteral("song"),
                                                 {{QStringLiteral("id"), trackId}})).object());
    if (!song) {
        throw SpotifyError::invalidData(QStringLiteral("YouTube Music returned an unsupported track."));
    }
    co_return *song;
}

QCoro::Task<Album> WebApi::album(QString albumId, QString accessToken)
{
    Q_UNUSED(accessToken)
    const auto album = json::album((co_await call(QStringLiteral("album"),
                                                  {{QStringLiteral("id"), albumId}})).object());
    if (!album) {
        throw SpotifyError::invalidData(QStringLiteral("YouTube Music returned an unsupported album."));
    }
    co_return *album;
}

QCoro::Task<Artist> WebApi::artist(QString artistId, QString accessToken)
{
    Q_UNUSED(accessToken)
    const auto artist = json::artist((co_await call(QStringLiteral("artist"),
                                                    {{QStringLiteral("id"), artistId}})).object());
    if (!artist) {
        throw SpotifyError::invalidData(QStringLiteral("YouTube Music returned an unsupported artist."));
    }
    co_return *artist;
}

QCoro::Task<Playlist> WebApi::playlist(QString playlistId, QString accessToken)
{
    Q_UNUSED(accessToken)
    const Playlist playlist = json::playlist((co_await call(QStringLiteral("playlist"),
                                                            {{QStringLiteral("id"), playlistId}})).object());
    if (!playlist.isValid()) {
        throw SpotifyError::invalidData(QStringLiteral("YouTube Music returned an unsupported playlist."));
    }
    co_return playlist;
}

QCoro::Task<QList<Song>> WebApi::albumSongs(Album album, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_return pageSongs((co_await call(QStringLiteral("albumSongs"),
                                       {{QStringLiteral("id"), album.id}})).object());
}

QCoro::Task<QList<Album>> WebApi::artistAlbums(Artist artist, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_return pageAlbums((co_await call(QStringLiteral("artistAlbums"),
                                        {{QStringLiteral("id"), artist.id}})).object());
}

QCoro::Task<QHash<QString, QStringList>> WebApi::artistGenres(QStringList artistIds, QString accessToken,
                                                              QHash<QString, QUrl> *artwork)
{
    Q_UNUSED(accessToken)
    QJsonArray ids;
    for (const QString &id : artistIds) {
        ids.append(id);
    }
    const QJsonObject response = (co_await call(QStringLiteral("artistGenres"),
                                                {{QStringLiteral("ids"), ids}})).object();
    QHash<QString, QStringList> result;
    for (const QJsonValue &value : response.value(QStringLiteral("artists")).toArray()) {
        const QJsonObject object = value.toObject();
        const QString id = object.value(QStringLiteral("id")).toString();
        if (id.isEmpty()) {
            continue;
        }
        QStringList genres;
        for (const QJsonValue &genre : object.value(QStringLiteral("genres")).toArray()) {
            genres.append(genre.toString());
        }
        result.insert(id, genres);
        if (artwork) {
            const QUrl image = json::preferredArtworkUrl(object.value(QStringLiteral("images")).toArray(), 320);
            if (image.isValid()) {
                artwork->insert(id, image);
            }
        }
    }
    co_return result;
}

QCoro::Task<Playlist> WebApi::createPlaylist(QString name, QString description, bool isPublic, QString accessToken)
{
    Q_UNUSED(accessToken)
    const Playlist playlist = json::playlist((co_await call(QStringLiteral("createPlaylist"),
        {{QStringLiteral("name"), name},
         {QStringLiteral("description"), description},
         {QStringLiteral("public"), isPublic}})).object());
    if (!playlist.isValid()) {
        throw SpotifyError::invalidData(QStringLiteral("the new playlist had no id."));
    }
    co_return playlist;
}

QCoro::Task<> WebApi::updatePlaylist(QString id, QString name, QString description, bool isPublic, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_await call(QStringLiteral("updatePlaylist"),
                  {{QStringLiteral("id"), id},
                   {QStringLiteral("name"), name},
                   {QStringLiteral("description"), description},
                   {QStringLiteral("public"), isPublic}});
}

QCoro::Task<> WebApi::addToPlaylist(QString songUri, QString playlistId, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_await call(QStringLiteral("addToPlaylist"),
                  {{QStringLiteral("uri"), songUri}, {QStringLiteral("playlistId"), playlistId}});
}

QCoro::Task<> WebApi::removeFromPlaylist(QString songUri, QString playlistId, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_await call(QStringLiteral("removeFromPlaylist"),
                  {{QStringLiteral("uri"), songUri}, {QStringLiteral("playlistId"), playlistId}});
}

QCoro::Task<> WebApi::reorderPlaylist(QString playlistId, int rangeStart, int insertBefore, int length,
                                      QString accessToken)
{
    Q_UNUSED(accessToken)
    co_await call(QStringLiteral("reorderPlaylist"),
                  {{QStringLiteral("playlistId"), playlistId},
                   {QStringLiteral("rangeStart"), rangeStart},
                   {QStringLiteral("insertBefore"), insertBefore},
                   {QStringLiteral("length"), length}});
}

QCoro::Task<> WebApi::setPlaylistImage(QString playlistId, QByteArray jpeg, QString accessToken)
{
    Q_UNUSED(playlistId)
    Q_UNUSED(jpeg)
    Q_UNUSED(accessToken)
    co_await call(QStringLiteral("setPlaylistImage"));
}

QCoro::Task<> WebApi::deletePlaylist(QString id, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_await call(QStringLiteral("deletePlaylist"), {{QStringLiteral("id"), id}});
}

QCoro::Task<> WebApi::setSaved(bool saved, QString uri, QString accessToken)
{
    Q_UNUSED(accessToken)
    co_await call(QStringLiteral("setSaved"),
                  {{QStringLiteral("saved"), saved}, {QStringLiteral("uri"), uri}});
}

QCoro::Task<QHash<QString, bool>> WebApi::savedStatuses(QStringList uris, QString accessToken)
{
    Q_UNUSED(accessToken)
    const QJsonObject response = (co_await call(QStringLiteral("savedStatuses"),
                                                {{QStringLiteral("uris"), uriArray(uris)}})).object();
    QHash<QString, bool> result;
    for (auto it = response.constBegin(); it != response.constEnd(); ++it) {
        result.insert(it.key(), it.value().toBool());
    }
    co_return result;
}

} // namespace swiftify
