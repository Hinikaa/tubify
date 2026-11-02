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

pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Dialogs
import Swiftify

// the system's own pickers for music to add to Local Files: a folder,
// searched all the way down, or a few files. given a playlist made here,
// what is picked is copied into it instead
Item {
    id: root

    property string playlistId

    function pickFolder() {
        folderDialog.open()
    }

    function pickFiles() {
        fileDialog.open()
    }

    function add(urls: list<url>) {
        if (playlistId.length > 0) {
            LocalFiles.addFiles(playlistId, urls)
            UiState.toastRequested(qsTr("Adding to the playlist…"))
            return
        }
        const added = LocalFiles.addSources(urls)
        UiState.toastRequested(added > 0 ? qsTr("Looking for music…") : qsTr("Already in Local Files"))
    }

    FolderDialog {
        id: folderDialog
        title: qsTr("Add a Folder of Music")
        onAccepted: root.add([selectedFolder])
    }

    FileDialog {
        id: fileDialog
        title: root.playlistId.length > 0 ? qsTr("Add Tracks") : qsTr("Add Music Files")
        fileMode: FileDialog.OpenFiles
        nameFilters: ["Music (*.mp3 *.flac *.ogg *.oga *.m4a *.mp4 *.aac *.wav *.aif *.aiff)"]
        onAccepted: root.add(selectedFiles)
    }
}
