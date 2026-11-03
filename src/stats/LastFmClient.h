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

#include "stats/StatsFmClient.h"
#include "stats/StatsTypes.h"

#include <QCoroTask>
#include <QJsonObject>
#include <QString>
#include <QUrlQuery>

class QNetworkAccessManager;

namespace swiftify {

/// Last.fm's public api: reads need a key, writes need a session key. the
/// default key/secret are Last.fm Desktop's public GPL pair, so reading and
/// scrobbling work without the person registering an application.
class LastFmClient {
public:
    explicit LastFmClient(QNetworkAccessManager *network);

    static QString normalizeUsername(const QString &raw);
    static QString periodName(StatsPeriod period);

    QCoro::Task<StatsFmProfile> profile(QString username, QString apiKey);
    QCoro::Task<StatsSnapshot> snapshot(QString username, StatsPeriod period, QString apiKey);

    /// a request token for the browser login, and the session key it becomes
    QCoro::Task<QString> requestToken();
    QCoro::Task<QString> sessionKey(QString token);
    static QString authUrl(const QString &token);
    /// one play, scrobbled with the session key
    QCoro::Task<> scrobble(QString sessionKey, QString artist, QString title, QString album,
                           int durationSeconds, qint64 timestamp);

    // --- the parts of a reply, public for tests ---------------------------
    static StatEntry track(const QJsonObject &entry);
    static StatEntry artist(const QJsonObject &entry);
    static StatEntry album(const QJsonObject &entry);
    static StatEntry tag(const QJsonObject &entry);
    static StatEntry recent(const QJsonObject &entry);

private:
    QCoro::Task<QJsonObject> get(QString method, QString apiKey, QUrlQuery query);
    QCoro::Task<QJsonObject> getSigned(QString method, QUrlQuery query, QString sessionKey = {});
    QCoro::Task<QJsonObject> getUrl(QUrl url, QByteArray body = {});
    QNetworkAccessManager *m_network;
};

} // namespace swiftify
