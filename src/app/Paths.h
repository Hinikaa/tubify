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

/// what the app keeps of the person's, such as their local playlists, created
/// on first use: $XDG_DATA_HOME/Tubify on linux, ~/Library/Application
/// Support/Tubify on macos.
QString dataDirectory();
/// for artwork and other things safe to delete: $XDG_CACHE_HOME/Tubify on
/// linux, ~/Library/Caches/Tubify on macos.
QString cacheDirectory();
/// for diagnostic output: dataDirectory()/logs on linux, ~/Library/Logs/Tubify
/// on macos, where console.app lists it.
QString logDirectory();
/// logDirectory()/tubify.log
QString logFilePath();

} // namespace swiftify::paths
