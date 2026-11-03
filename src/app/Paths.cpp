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

#include "app/Paths.h"

#include <QDir>
#include <QStandardPaths>

namespace swiftify::paths {

namespace {

QString ensured(const QString &path)
{
    QDir().mkpath(path);
    return path;
}

/// a writable location the system gives, with Tubify's own folder inside it
/// and `tail` under that. the generic locations are the ones that do not wait
/// on the application's name being set: on windows they are the local app data
/// folder, and on linux $XDG_DATA_HOME and $XDG_CACHE_HOME
QString location(QStandardPaths::StandardLocation kind, const QString &fallback, const QString &tail = {})
{
    QString base = QStandardPaths::writableLocation(kind);
    if (base.isEmpty()) {
        base = fallback;
    }
    return ensured(base + QStringLiteral("/Tubify") + tail);
}

} // namespace

QString dataDirectory()
{
    // %LOCALAPPDATA%\Tubify on windows, $XDG_DATA_HOME/Tubify on linux,
    // ~/Library/Application Support/Tubify on macos: what the person made
    // here, such as their local playlists and their history
    return location(QStandardPaths::GenericDataLocation,
                    QDir::homePath() + QStringLiteral("/.local/share"));
}

QString cacheDirectory()
{
#ifdef Q_OS_WIN
    // windows gives the same folder for the generic cache and data
    // locations, so the cache takes one of its own inside it: clearing it must
    // not take the playlists with it
    return location(QStandardPaths::GenericCacheLocation,
                    QDir::homePath() + QStringLiteral("/AppData/Local"), QStringLiteral("/cache"));
#else
    return location(QStandardPaths::GenericCacheLocation,
                    QDir::homePath() + QStringLiteral("/.cache"));
#endif
}

QString logDirectory()
{
#ifdef Q_OS_MACOS
    // where a mac app's logs go: console.app lists the folder, and the system
    // keeps it out of the app's own data
    return ensured(QDir::homePath() + QStringLiteral("/Library/Logs/Tubify"));
#else
    return ensured(dataDirectory() + QStringLiteral("/logs"));
#endif
}

QString logFilePath()
{
    return logDirectory() + QStringLiteral("/tubify.log");
}

} // namespace swiftify::paths
