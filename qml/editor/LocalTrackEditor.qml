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

// what a music file of one's own says about itself, written into the file:
// its cover (a photo framed in the cropper, or none), title, artists, album,
// album artist, genre, year, track and disc. nothing is written until Save,
// and then into a copy that takes the file's place only once it is whole.
StyledDialog {
    id: root

    property song song
    /// a photo cropped and encoded, waiting for Save
    property var pickedCover: null
    /// the cover goes, unless a new one is picked
    property bool removingCover: false
    property url currentCover
    property bool saving: false
    property string problem

    readonly property real coverEdge: 120

    function openFor(target: song) {
        song = target
        const tags = LocalFiles.tagsOf(target.uri)
        titleField.text = tags.title ?? ""
        artistField.text = tags.artist ?? ""
        albumField.text = tags.album ?? ""
        albumArtistField.text = tags.albumArtist ?? ""
        genreField.text = tags.genre ?? ""
        dateField.text = tags.date ?? ""
        trackField.text = tags.trackNumber ?? ""
        discField.text = tags.discNumber ?? ""
        currentCover = tags.artworkUrl ?? ""
        pickedCover = null
        removingCover = false
        saving = false
        problem = ""
        maker.photo = ""
        open()
        titleField.inputItem.forceActiveFocus()
    }

    function save() {
        if (saving) {
            return
        }
        saving = true
        problem = ""
        LocalFiles.editTags(song.uri, {
            title: titleField.text,
            artist: artistField.text,
            album: albumField.text,
            albumArtist: albumArtistField.text,
            genre: genreField.text,
            date: dateField.text,
            trackNumber: trackField.text,
            discNumber: discField.text
        }, pickedCover ?? new ArrayBuffer(0), removingCover && pickedCover === null)
    }

    headline: qsTr("Edit Info")
    symbol: "edit_note"
    iconShape: "puffy"
    preferredWidth: 480

    Connections {
        target: LocalFiles
        enabled: root.saving
        function onTagsSaved(uri: string, ok: bool, error: string) {
            if (uri !== root.song.uri) {
                return
            }
            root.saving = false
            if (ok) {
                UiState.toastRequested(qsTr("Saved to the file"))
                root.accept()
            } else {
                root.problem = error
            }
        }
    }

    CoverMaker {
        id: maker
    }

    FileDialog {
        id: picker
        title: qsTr("Choose Cover Art")
        nameFilters: ["Images (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.tif *.tiff *.avif *.heic)"]
        onAccepted: {
            maker.photo = selectedFile
            if (maker.photoValid) {
                root.problem = ""
                cropper.begin()
            } else {
                root.problem = qsTr("That image could not be used as cover art.")
            }
        }
    }

    CoverCropper {
        id: cropper
        maker: maker
        onUsed: (jpeg) => {
            root.pickedCover = jpeg
            root.removingCover = false
        }
    }

    Row {
        width: parent.width
        spacing: 16

        // the cover as it will be: a new one, none, or the file's own
        Item {
            width: root.coverEdge
            height: root.coverEdge

            ArtworkImage {
                anchors.fill: parent
                radius: 12
                visible: root.pickedCover === null
                source: root.removingCover ? "" : root.currentCover
                placeholderSymbol: "music_note"
            }
            // the cropped photo, straight from its jpeg rather than through
            // the artwork cache, which loads only files and the web
            Image {
                id: picked
                visible: false
                source: root.pickedCover !== null ? maker.preview(root.pickedCover) : ""
            }
            ShaderEffect {
                anchors.fill: parent
                visible: root.pickedCover !== null && picked.status === Image.Ready

                property variant source: picked
                property size itemSize: Qt.size(width, height)
                property real radius: 12
                property real borderWidth: 0
                property color borderColor: "transparent"
                property size uvScale: Qt.size(1, 1)
                property point uvOffset: Qt.point(0, 0)

                property real waviness: 0
                property real wavelength: 22
                property real phase: 0
                property real ringWaves: 0
                property real ringAmplitude: 0
                property real ringThickness: 0
                property real ringPhase: 0
                property color ringColor: "transparent"
                property real reveal: 1

                fragmentShader: "qrc:/shaders/rounded_image.frag.qsb"
            }
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            TextButton {
                variant: TextButton.Tonal
                symbol: "add_photo_alternate"
                text: root.pickedCover !== null || (root.currentCover.toString().length > 0 && !root.removingCover)
                      ? qsTr("Change Cover…") : qsTr("Add Cover…")
                onClicked: picker.open()
            }
            TextButton {
                variant: TextButton.Text
                symbol: "hide_image"
                text: qsTr("Remove Cover")
                visible: root.pickedCover !== null || (root.currentCover.toString().length > 0 && !root.removingCover)
                onClicked: {
                    root.pickedCover = null
                    root.removingCover = true
                }
            }
        }
    }

    StyledText {
        width: parent.width
        Reveal.shown: root.problem.length > 0
        text: root.problem
        wrapMode: Text.Wrap
        font.pixelSize: Appearance.fontSize.smaller
        color: Appearance.colors.error
    }

    OutlinedTextField {
        id: titleField
        width: parent.width
        placeholder: qsTr("Title")
        onAccepted: root.save()
    }

    OutlinedTextField {
        id: artistField
        width: parent.width
        placeholder: qsTr("Artist")
        onAccepted: root.save()
    }

    OutlinedTextField {
        id: albumField
        width: parent.width
        placeholder: qsTr("Album")
        onAccepted: root.save()
    }

    OutlinedTextField {
        id: albumArtistField
        width: parent.width
        placeholder: qsTr("Album Artist")
        onAccepted: root.save()
    }

    Row {
        width: parent.width
        spacing: 12

        OutlinedTextField {
            id: genreField
            width: (parent.width - parent.spacing) / 2
            placeholder: qsTr("Genre")
            onAccepted: root.save()
        }
        OutlinedTextField {
            id: dateField
            width: (parent.width - parent.spacing) / 2
            placeholder: qsTr("Year")
            onAccepted: root.save()
        }
    }

    Row {
        width: parent.width
        spacing: 12

        OutlinedTextField {
            id: trackField
            width: (parent.width - parent.spacing) / 2
            placeholder: qsTr("Track")
            inputItem.inputMethodHints: Qt.ImhDigitsOnly
            onAccepted: root.save()
        }
        OutlinedTextField {
            id: discField
            width: (parent.width - parent.spacing) / 2
            placeholder: qsTr("Disc")
            inputItem.inputMethodHints: Qt.ImhDigitsOnly
            onAccepted: root.save()
        }
    }

    buttons: [
        TextButton {
            text: qsTr("Save")
            busy: root.saving
            enabled: !root.saving
            onClicked: root.save()
        },
        TextButton {
            variant: TextButton.Text
            text: qsTr("Cancel")
            onClicked: root.close()
        }
    ]
}
