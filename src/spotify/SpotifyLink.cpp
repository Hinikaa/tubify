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

#include "spotify/SpotifyLink.h"

#include <QRegularExpression>
#include <QStringList>
#include <QUrl>
#include <QUrlQuery>

namespace swiftify {

namespace {

std::optional<SpotifyLink::Kind> kindFromName(const QString &name)
{
    if (name == QLatin1String("track")) {
        return SpotifyLink::Kind::Track;
    }
    if (name == QLatin1String("album")) {
        return SpotifyLink::Kind::Album;
    }
    if (name == QLatin1String("playlist")) {
        return SpotifyLink::Kind::Playlist;
    }
    if (name == QLatin1String("artist")) {
        return SpotifyLink::Kind::Artist;
    }
    return std::nullopt;
}

std::optional<SpotifyLink> fromParts(const QString &kindName, const QString &id)
{
    static const QRegularExpression videoPattern(QStringLiteral("^[A-Za-z0-9_-]{6,}$"));
    static const QRegularExpression albumPattern(QStringLiteral("^MPRE[A-Za-z0-9_-]+$"));
    static const QRegularExpression artistPattern(QStringLiteral("^UC[A-Za-z0-9_-]+$"));
    static const QRegularExpression playlistPattern(QStringLiteral("^(VL|PL|OLAK|RD|LM|SE)[A-Za-z0-9_-]*$"));
    const auto kind = kindFromName(kindName);
    if (!kind) {
        return std::nullopt;
    }
    const bool valid = *kind == SpotifyLink::Kind::Track      ? videoPattern.match(id).hasMatch()
        : *kind == SpotifyLink::Kind::Album                   ? albumPattern.match(id).hasMatch()
        : *kind == SpotifyLink::Kind::Artist                  ? artistPattern.match(id).hasMatch()
                                                              : playlistPattern.match(id).hasMatch();
    if (!valid) {
        return std::nullopt;
    }
    return SpotifyLink {*kind, id};
}

std::optional<SpotifyLink> fromBrowseId(const QString &browseId)
{
    if (browseId.startsWith(QLatin1String("MPRE"))) {
        return SpotifyLink {SpotifyLink::Kind::Album, browseId};
    }
    if (browseId.startsWith(QLatin1String("UC"))) {
        return SpotifyLink {SpotifyLink::Kind::Artist, browseId};
    }
    if (browseId.startsWith(QLatin1String("VL"))) {
        return SpotifyLink {SpotifyLink::Kind::Playlist, browseId.mid(2)};
    }
    return std::nullopt;
}

} // namespace

std::optional<SpotifyLink> parseSpotifyLink(const QString &input)
{
    const QString text = input.trimmed();

    if (text.startsWith(QLatin1String("ytm:"))) {
        const QStringList parts = text.split(QLatin1Char(':'));
        if (parts.size() != 3) {
            return std::nullopt;
        }
        return fromParts(parts[1], parts[2]);
    }

    const QUrl url(text);
    if (url.scheme() != QLatin1String("https") && url.scheme() != QLatin1String("http")) {
        return std::nullopt;
    }

    if (url.host() == QLatin1String("youtu.be")) {
        const QString id = url.path().mid(1).section(QLatin1Char('/'), 0, 0);
        return fromParts(QStringLiteral("track"), id);
    }
    if (url.host() != QLatin1String("music.youtube.com") && url.host() != QLatin1String("www.youtube.com")
        && url.host() != QLatin1String("youtube.com")) {
        return std::nullopt;
    }

    const QUrlQuery query(url);
    if (query.hasQueryItem(QStringLiteral("v"))) {
        return fromParts(QStringLiteral("track"), query.queryItemValue(QStringLiteral("v")));
    }
    if (query.hasQueryItem(QStringLiteral("list"))) {
        QString list = query.queryItemValue(QStringLiteral("list"));
        if (list.startsWith(QLatin1String("VL"))) {
            list = list.mid(2);
        }
        return fromParts(QStringLiteral("playlist"), list);
    }

    QStringList segments = url.path().split(QLatin1Char('/'), Qt::SkipEmptyParts);
    if (!segments.isEmpty() && segments.first() == QLatin1String("browse") && segments.size() == 2) {
        return fromBrowseId(segments.at(1));
    }
    if (!segments.isEmpty() && segments.first() == QLatin1String("channel") && segments.size() == 2) {
        return fromParts(QStringLiteral("artist"), segments.at(1));
    }
    if (segments.size() == 2 && (segments.first() == QLatin1String("playlist")
                                 || segments.first() == QLatin1String("album"))) {
        return fromParts(segments.first(), segments.at(1));
    }
    return std::nullopt;
}

} // namespace swiftify
