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

#include "playback/Lyrics.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>

#include <algorithm>

namespace swiftify {

std::optional<Lyrics> Lyrics::fromLrc(const QString &text)
{
    // [mm:ss], [mm:ss.xx] or [mm:ss.xxx], one or more ahead of the words
    static const QRegularExpression stamp(QStringLiteral(R"(^\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\])"));
    // [ar:...], [ti:...] and the like say who and what, and are not sung
    static const QRegularExpression tag(QStringLiteral(R"(^\[[a-zA-Z#]+:.*\]\s*$)"));

    Lyrics lyrics;
    lyrics.provider = QStringLiteral("LRC file");
    QList<LyricLine> timed;
    QList<LyricLine> plain;
    for (QString line : text.split(u'\n')) {
        line = line.trimmed();
        QList<quint32> starts;
        for (auto match = stamp.match(line); match.hasMatch(); match = stamp.match(line)) {
            const QString fraction = match.captured(3);
            const quint32 milliseconds = fraction.isEmpty() ? 0
                : quint32(fraction.toUInt() * (fraction.size() == 1 ? 100 : fraction.size() == 2 ? 10 : 1));
            starts.append(quint32(match.captured(1).toUInt() * 60'000 + match.captured(2).toUInt() * 1000) + milliseconds);
            line = line.mid(match.capturedLength()).trimmed();
        }
        if (!starts.isEmpty()) {
            for (const quint32 start : starts) {
                timed.append({start, 0, line});
            }
        } else if (!line.isEmpty() && !tag.match(line).hasMatch()) {
            plain.append({0, 0, line});
        }
    }
    if (!timed.isEmpty()) {
        std::stable_sort(timed.begin(), timed.end(),
                         [](const LyricLine &a, const LyricLine &b) { return a.startTimeMs < b.startTimeMs; });
        for (qsizetype index = 0; index + 1 < timed.size(); ++index) {
            timed[index].endTimeMs = timed[index + 1].startTimeMs;
        }
        timed.last().endTimeMs = timed.last().startTimeMs + 5000;
        lyrics.synchronized = true;
        lyrics.lines = timed;
    } else {
        lyrics.lines = plain;
    }
    if (lyrics.lines.isEmpty()) {
        return std::nullopt;
    }
    return lyrics;
}

std::optional<Lyrics> Lyrics::besideFile(const QString &musicPath)
{
    const QFileInfo music(musicPath);
    // "song.mp3" goes with "song.lrc". named outright rather than matched by
    // a pattern: a name like "Injury [HQ]" is a pattern of its own
    for (const char *suffix : {".lrc", ".LRC", ".Lrc"}) {
        QFile file(music.dir().filePath(music.completeBaseName() + QLatin1StringView(suffix)));
        if (file.exists() && file.size() < 512 * 1024 && file.open(QIODevice::ReadOnly)) {
            return fromLrc(QString::fromUtf8(file.readAll()));
        }
    }
    return std::nullopt;
}

std::optional<Lyrics> Lyrics::fromJson(const QByteArray &json)
{
    QJsonParseError error;
    const QJsonObject object = QJsonDocument::fromJson(json, &error).object();
    if (error.error != QJsonParseError::NoError || !object.contains(QStringLiteral("lines"))) {
        return std::nullopt;
    }
    Lyrics lyrics;
    lyrics.language = object.value(QStringLiteral("language")).toString();
    lyrics.provider = object.value(QStringLiteral("provider")).toString();
    lyrics.synchronized = object.value(QStringLiteral("syncType")).toString() == QLatin1StringView("lineSynced");
    for (const QJsonValue &value : object.value(QStringLiteral("lines")).toArray()) {
        const QJsonObject line = value.toObject();
        lyrics.lines.append({
            quint32(line.value(QStringLiteral("startTimeMs")).toDouble()),
            quint32(line.value(QStringLiteral("endTimeMs")).toDouble()),
            line.value(QStringLiteral("words")).toString(),
        });
    }
    // spotify gives synced lines no end, so each lasts until the next one
    // starts and the last for a few seconds
    for (qsizetype index = 0; index < lyrics.lines.size(); ++index) {
        LyricLine &line = lyrics.lines[index];
        if (line.endTimeMs <= line.startTimeMs) {
            line.endTimeMs = index + 1 < lyrics.lines.size() ? lyrics.lines.at(index + 1).startTimeMs
                                                             : line.startTimeMs + 5000;
        }
    }
    return lyrics;
}

int Lyrics::lineIndex(quint32 positionMs) const
{
    if (!synchronized || lines.isEmpty()) {
        return -1;
    }
    qsizetype lower = 0;
    qsizetype upper = lines.size();
    while (lower < upper) {
        const qsizetype middle = lower + (upper - lower) / 2;
        if (lines.at(middle).startTimeMs <= positionMs) {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    return int(lower) - 1;
}

LyricsModel::LyricsModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int LyricsModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_lines.size());
}

QVariant LyricsModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_lines.size()) {
        return {};
    }
    const LyricLine &line = m_lines.at(index.row());
    switch (role) {
    case WordsRole: return line.words;
    case StartTimeMsRole: return line.startTimeMs;
    case EndTimeMsRole: return line.endTimeMs;
    default: return {};
    }
}

QHash<int, QByteArray> LyricsModel::roleNames() const
{
    return {{WordsRole, "words"}, {StartTimeMsRole, "startTimeMs"}, {EndTimeMsRole, "endTimeMs"}};
}

void LyricsModel::setLines(const QList<LyricLine> &lines)
{
    beginResetModel();
    m_lines = lines;
    endResetModel();
    emit countChanged();
}

} // namespace swiftify
