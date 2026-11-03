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

#pragma once

#include <QString>

namespace swiftify::paths {

/// %LOCALAPPDATA%\Tubify on windows, $XDG_DATA_HOME/Tubify on linux, and
/// ~/Library/Application Support/Tubify on macos: what the app keeps of the
/// person's, such as their local playlists and listening history.
QString dataDirectory();
/// the system's cache folder with Tubify inside it, for artwork and songs
/// kept to play again: things safe to delete, kept apart from the data.
QString cacheDirectory();
/// for diagnostic output: dataDirectory()/logs on windows and linux,
/// ~/Library/Logs/Tubify on macos, where console.app lists it.
QString logDirectory();
/// logDirectory()/tubify.log
QString logFilePath();

} // namespace swiftify::paths
