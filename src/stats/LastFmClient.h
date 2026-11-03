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

/// Last.fm's public read api: a username and the user's own api key are all
/// it needs. it answers in the same snapshot the stats page already draws.
class LastFmClient {
public:
    explicit LastFmClient(QNetworkAccessManager *network);

    /// trimmed, a profile link reduced to its last part, a leading @ removed
    static QString normalizeUsername(const QString &raw);
    /// Last.fm's name for a period: 7day, 1month, 6month or overall
    static QString periodName(StatsPeriod period);

    QCoro::Task<StatsFmProfile> profile(QString username, QString apiKey);
    QCoro::Task<StatsSnapshot> snapshot(QString username, StatsPeriod period, QString apiKey);

    // --- the parts of a reply, public for tests ---------------------------
    static StatEntry track(const QJsonObject &entry);
    static StatEntry artist(const QJsonObject &entry);
    static StatEntry album(const QJsonObject &entry);
    static StatEntry tag(const QJsonObject &entry);
    static StatEntry recent(const QJsonObject &entry);

private:
    QCoro::Task<QJsonObject> get(QString method, QString apiKey, QUrlQuery query);
    QNetworkAccessManager *m_network;
};

} // namespace swiftify
