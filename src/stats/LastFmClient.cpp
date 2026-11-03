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

#include "stats/LastFmClient.h"

#include <QDate>
#include <QDateTime>
#include <QTime>
#include <QJsonArray>
#include <QJsonDocument>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QRegularExpression>
#include <QStringList>
#include <QUrl>
#include <QCoroNetworkReply>

#include <algorithm>
#include <memory>
#include <utility>

namespace swiftify {

namespace {

const QString kApiBase = QStringLiteral("https://ws.audioscrobbler.com/2.0/");
// MusicBrainz Picard's public app key: a shared default so Last.fm works
// without the person registering an application. a key in Settings overrides it.
const QString kDefaultApiKey = QStringLiteral("0a210a4a6741f2ec8f27a791b9d5d971");

QString textOf(const QJsonValue &value)
{
    if (value.isString()) {
        return value.toString();
    }
    const QJsonObject object = value.toObject();
    const QString text = object.value(QStringLiteral("#text")).toString();
    return text.isEmpty() ? object.value(QStringLiteral("name")).toString() : text;
}

int intOf(const QJsonValue &value)
{
    if (value.isDouble()) {
        return value.toInt();
    }
    return value.toString().toInt();
}

qint64 longOf(const QJsonValue &value)
{
    if (value.isDouble()) {
        return qint64(value.toDouble());
    }
    return value.toString().toLongLong();
}

QUrl imageUrl(const QJsonArray &images)
{
    static const QStringList order {
        QStringLiteral("extralarge"), QStringLiteral("large"), QStringLiteral("medium"), QStringLiteral("small")
    };
    for (const QString &size : order) {
        for (const QJsonValue &value : images) {
            const QJsonObject image = value.toObject();
            if (image.value(QStringLiteral("size")).toString() == size) {
                const QString url = image.value(QStringLiteral("#text")).toString();
                if (!url.isEmpty()) {
                    return QUrl(url);
                }
            }
        }
    }
    for (const QJsonValue &value : images) {
        const QString url = value.toObject().value(QStringLiteral("#text")).toString();
        if (!url.isEmpty()) {
            return QUrl(url);
        }
    }
    return {};
}

QString artistName(const QJsonObject &entry)
{
    return textOf(entry.value(QStringLiteral("artist")));
}

QString albumName(const QJsonObject &entry)
{
    return textOf(entry.value(QStringLiteral("album")));
}

} // namespace

LastFmClient::LastFmClient(QNetworkAccessManager *network)
    : m_network(network)
{
}

QString LastFmClient::normalizeUsername(const QString &raw)
{
    QString name = raw.trimmed();
    if (name.contains(u'/')) {
        const QStringList parts = QUrl::fromUserInput(name).path().split(u'/', Qt::SkipEmptyParts);
        name = parts.isEmpty() ? QString() : parts.last();
    }
    if (name.startsWith(u'@')) {
        name.remove(0, 1);
    }
    static const QRegularExpression valid(QStringLiteral("^[A-Za-z0-9._-]{1,64}$"));
    return valid.match(name).hasMatch() ? name : QString();
}

QString LastFmClient::periodName(StatsPeriod period)
{
    switch (period) {
    case StatsPeriod::Today: return QStringLiteral("7day");
    case StatsPeriod::Weeks: return QStringLiteral("1month");
    case StatsPeriod::Months: return QStringLiteral("6month");
    case StatsPeriod::Lifetime: return QStringLiteral("overall");
    }
    return QStringLiteral("1month");
}

QCoro::Task<QJsonObject> LastFmClient::get(QString method, QString apiKey, QUrlQuery query)
{
    QUrl url(kApiBase);
    query.addQueryItem(QStringLiteral("method"), method);
    query.addQueryItem(QStringLiteral("api_key"), apiKey.trimmed().isEmpty() ? kDefaultApiKey : apiKey);
    query.addQueryItem(QStringLiteral("format"), QStringLiteral("json"));
    url.setQuery(query);

    QNetworkRequest request(url);
    request.setRawHeader("Accept", "application/json");
    request.setTransferTimeout(15'000);

    std::unique_ptr<QNetworkReply> reply(m_network->get(request));
    co_await qCoro(reply.get()).waitForFinished();

    const int status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    const QByteArray data = reply->readAll();
    if (status == 0) {
        throw StatsFmError(StatsFmError::Kind::Network,
                           QStringLiteral("Could not reach Last.fm: %1").arg(reply->errorString()));
    }
    if (status < 200 || status >= 300) {
        throw StatsFmError(StatsFmError::Kind::Other,
                           QStringLiteral("Last.fm answered with HTTP %1.").arg(status));
    }

    const QJsonObject object = QJsonDocument::fromJson(data).object();
    if (object.contains(QStringLiteral("error"))) {
        const int code = object.value(QStringLiteral("error")).toInt();
        const QString message = object.value(QStringLiteral("message")).toString();
        switch (code) {
        case 6:
            throw StatsFmError(StatsFmError::Kind::NotFound,
                               QStringLiteral("Last.fm has no user by that name."));
        case 10:
        case 13:
            throw StatsFmError(StatsFmError::Kind::Other,
                               QStringLiteral("Last.fm rejected the API key. Check it in Settings."));
        case 17:
            throw StatsFmError(StatsFmError::Kind::Private,
                               QStringLiteral("This Last.fm user keeps their listening private."));
        case 29:
            throw StatsFmError(StatsFmError::Kind::RateLimited,
                               QStringLiteral("Last.fm is busy. Try again in a minute."));
        default:
            throw StatsFmError(StatsFmError::Kind::Other,
                               message.isEmpty() ? QStringLiteral("Last.fm refused the request.") : message);
        }
    }
    co_return object;
}

QCoro::Task<StatsFmProfile> LastFmClient::profile(QString username, QString apiKey)
{
    QUrlQuery query;
    query.addQueryItem(QStringLiteral("user"), username);
    const QJsonObject user = (co_await get(QStringLiteral("user.getInfo"), apiKey, query))
                                 .value(QStringLiteral("user")).toObject();
    StatsFmProfile profile;
    profile.id = username;
    profile.displayName = user.value(QStringLiteral("realname")).toString();
    if (profile.displayName.isEmpty()) {
        profile.displayName = user.value(QStringLiteral("name")).toString(username);
    }
    profile.imageUrl = imageUrl(user.value(QStringLiteral("image")).toArray());
    profile.isPlus = true;
    co_return profile;
}

QCoro::Task<StatsSnapshot> LastFmClient::snapshot(QString username, StatsPeriod period, QString apiKey)
{
    QUrlQuery top;
    top.addQueryItem(QStringLiteral("user"), username);
    top.addQueryItem(QStringLiteral("period"), periodName(period));
    top.addQueryItem(QStringLiteral("limit"), QStringLiteral("50"));

    QUrlQuery recentQuery;
    recentQuery.addQueryItem(QStringLiteral("user"), username);
    recentQuery.addQueryItem(QStringLiteral("limit"), QStringLiteral("200"));
    const QDateTime from = periodStart(period, QDateTime::currentDateTime());
    if (from.isValid()) {
        recentQuery.addQueryItem(QStringLiteral("from"), QString::number(from.toSecsSinceEpoch()));
        recentQuery.addQueryItem(QStringLiteral("to"), QString::number(QDateTime::currentDateTime().toSecsSinceEpoch()));
    }

    auto tracksTask = get(QStringLiteral("user.getTopTracks"), apiKey, top);
    auto artistsTask = get(QStringLiteral("user.getTopArtists"), apiKey, top);
    auto albumsTask = get(QStringLiteral("user.getTopAlbums"), apiKey, top);
    auto tagsTask = get(QStringLiteral("user.getTopTags"), apiKey, top);
    auto recentTask = get(QStringLiteral("user.getRecentTracks"), apiKey, recentQuery);

    StatsSnapshot snapshot;
    for (const QJsonValue &value : (co_await tracksTask).value(QStringLiteral("toptracks"))
                                        .toObject().value(QStringLiteral("track")).toArray()) {
        if (StatEntry entry = track(value.toObject()); !entry.name.isEmpty()) {
            snapshot.listenedMs += entry.listenedMs;
            snapshot.topTracks.append(entry);
        }
    }
    for (const QJsonValue &value : (co_await artistsTask).value(QStringLiteral("topartists"))
                                        .toObject().value(QStringLiteral("artist")).toArray()) {
        if (StatEntry entry = artist(value.toObject()); !entry.name.isEmpty()) {
            snapshot.plays += entry.plays;
            snapshot.topArtists.append(entry);
        }
    }
    for (const QJsonValue &value : (co_await albumsTask).value(QStringLiteral("topalbums"))
                                        .toObject().value(QStringLiteral("album")).toArray()) {
        if (StatEntry entry = album(value.toObject()); !entry.name.isEmpty()) {
            snapshot.topAlbums.append(entry);
        }
    }
    for (const QJsonValue &value : (co_await tagsTask).value(QStringLiteral("toptags"))
                                        .toObject().value(QStringLiteral("tag")).toArray()) {
        if (StatEntry entry = tag(value.toObject()); !entry.name.isEmpty()) {
            snapshot.topGenres.append(entry);
        }
    }

    for (const QJsonValue &value : (co_await recentTask).value(QStringLiteral("recenttracks"))
                                        .toObject().value(QStringLiteral("track")).toArray()) {
        StatEntry entry = recent(value.toObject());
        if (entry.name.isEmpty()) {
            continue;
        }
        if (entry.playedAt.isValid()) {
            const QDateTime local = entry.playedAt.toLocalTime();
            snapshot.hourly[local.time().hour()]++;
            snapshot.weekdays[local.date().dayOfWeek() - 1]++;
            snapshot.daily[local.date()]++;
        }
        snapshot.recent.append(entry);
    }

    snapshot.uniqueTracks = snapshot.topTracks.size();
    snapshot.uniqueArtists = snapshot.topArtists.size();
    snapshot.uniqueAlbums = snapshot.topAlbums.size();
    snapshot.activeDays = snapshot.daily.size();
    snapshot.streakDays = streakEndingAt(snapshot.daily, QDate::currentDate());
    if (snapshot.plays == 0) {
        for (const StatEntry &entry : std::as_const(snapshot.topTracks)) {
            snapshot.plays += entry.plays;
        }
    }
    co_return snapshot;
}

StatEntry LastFmClient::track(const QJsonObject &entry)
{
    StatEntry stat;
    stat.name = entry.value(QStringLiteral("name")).toString();
    stat.subtitle = artistName(entry);
    stat.key = stat.name + u'|' + stat.subtitle;
    stat.plays = intOf(entry.value(QStringLiteral("playcount")));
    stat.artworkUrl = imageUrl(entry.value(QStringLiteral("image")).toArray());
    stat.listenedMs = longOf(entry.value(QStringLiteral("duration"))) * 1000 * std::max(stat.plays, 1);
    stat.song.name = stat.name;
    stat.song.artists = stat.subtitle;
    stat.song.albumName = albumName(entry);
    stat.song.artworkUrl = stat.artworkUrl;
    stat.song.isPlayable = false;
    return stat;
}

StatEntry LastFmClient::artist(const QJsonObject &entry)
{
    StatEntry stat;
    stat.name = entry.value(QStringLiteral("name")).toString();
    stat.key = stat.name;
    stat.plays = intOf(entry.value(QStringLiteral("playcount")));
    stat.artworkUrl = imageUrl(entry.value(QStringLiteral("image")).toArray());
    stat.artist.name = stat.name;
    stat.artist.artworkUrl = stat.artworkUrl;
    return stat;
}

StatEntry LastFmClient::album(const QJsonObject &entry)
{
    StatEntry stat;
    stat.name = entry.value(QStringLiteral("name")).toString();
    stat.subtitle = artistName(entry);
    stat.key = stat.name + u'|' + stat.subtitle;
    stat.plays = intOf(entry.value(QStringLiteral("playcount")));
    stat.artworkUrl = imageUrl(entry.value(QStringLiteral("image")).toArray());
    stat.album.name = stat.name;
    stat.album.artists = stat.subtitle;
    stat.album.artworkUrl = stat.artworkUrl;
    return stat;
}

StatEntry LastFmClient::tag(const QJsonObject &entry)
{
    StatEntry stat;
    stat.name = entry.value(QStringLiteral("name")).toString();
    stat.key = stat.name;
    stat.plays = intOf(entry.value(QStringLiteral("count")));
    return stat;
}

StatEntry LastFmClient::recent(const QJsonObject &entry)
{
    StatEntry stat;
    stat.name = entry.value(QStringLiteral("name")).toString();
    stat.subtitle = artistName(entry);
    stat.key = stat.name + u'|' + stat.subtitle;
    stat.plays = 1;
    stat.artworkUrl = imageUrl(entry.value(QStringLiteral("image")).toArray());
    const qint64 seconds = longOf(entry.value(QStringLiteral("date")).toObject().value(QStringLiteral("uts")));
    if (seconds > 0) {
        stat.playedAt = QDateTime::fromSecsSinceEpoch(seconds);
    }
    stat.song.name = stat.name;
    stat.song.artists = stat.subtitle;
    stat.song.albumName = albumName(entry);
    stat.song.artworkUrl = stat.artworkUrl;
    stat.song.isPlayable = false;
    return stat;
}

} // namespace swiftify
