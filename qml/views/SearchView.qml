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
import QtQuick.Controls.Basic
import Swiftify

// Search (GlobalSearchView): categories and history when empty, results after.
Flickable {
    id: root

    // pages run up under the top bar, which floats over them with no
    // background of its own; they start below it
    topMargin: Appearance.sizes.topBarHeight

    readonly property real margin: Appearance.sizes.pageMargin
    readonly property bool empty: Library.searchQuery.trim().length === 0

    contentHeight: column.implicitHeight + 24 + Appearance.sizes.playerBarClearance
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: StyledScrollBar { topPadding: 3 + Appearance.sizes.topBarHeight }

    /// whether something `top` down the page and `height` tall is in sight,
    /// or within `slack` of it
    function reaches(top: real, height: real, slack: real): bool {
        return top + height > root.contentY - slack && top < root.contentY + root.height + slack
    }

    Genres {
        id: genres
    }

    Stickers {
        width: root.width
        height: 220
        key: "search"
        count: 4
    }

    Column {
        id: column
        y: 24
        width: root.width
        spacing: 0

        // nothing typed: what was played from here, and categories
        Column {
            width: root.width
            Reveal.shown: root.empty

            Item {
                x: root.margin
                width: root.width - 2 * root.margin
                height: 48
                Reveal.shown: Library.searchHistory.count > 0

                TitleText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Played from Search")
                    font.pixelSize: Appearance.fontSize.title
                    font.weight: Font.Bold
                }
                TextButton {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    variant: TextButton.Text
                    text: qsTr("Clear")
                    onClicked: Library.clearSearchHistory()
                }
            }

            Repeater {
                model: root.empty ? Library.searchHistoryPreview : null

                SongRow {
                    x: root.margin
                    width: root.width - 2 * root.margin
                    songs: Library.searchHistoryPreview
                    showsNumber: false
                    fromSearch: true
                }
            }

            StyledText {
                x: root.margin
                topPadding: Library.searchHistory.count > 0 ? 26 : 0
                bottomPadding: 18
                text: qsTr("Browse Categories")
                font.pixelSize: 32
                font.weight: Font.Bold
            }

            Flow {
                x: root.margin
                width: root.width - 2 * root.margin
                spacing: 16

                Repeater {
                    model: genres.featured

                    GenreCard {
                        required property var modelData
                        name: modelData.name
                        symbol: modelData.symbol
                        startColor: modelData.start
                        endColor: modelData.end
                        onClicked: Library.searchQuery = name
                    }
                }
            }
        }

        // the phrase
        Item {
            width: root.width
            height: 360
            Reveal.shown: !root.empty && Library.searchIsCatchphrase

            Column {
                anchors.centerIn: parent
                spacing: 16

                MaterialSymbol {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 48
                    height: 48
                    text: "auto_awesome"
                    iconSize: 42
                    fill: 1
                    color: Appearance.colors.primary
                }
                TitleText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: UiState.catchphrase
                    font.pixelSize: Appearance.fontSize.headline
                    font.weight: Font.Bold
                }
                StyledText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: qsTr("You found it.")
                    color: Appearance.colors.onSurfaceVariant
                }
            }
        }

        Item {
            width: root.width
            height: 280
            Reveal.shown: !root.empty && !Library.searchIsCatchphrase && Library.searching

            Column {
                anchors.centerIn: parent
                spacing: 12
                LoadingSpinner { anchors.horizontalCenter: parent.horizontalCenter }
                StyledText {
                    text: qsTr("Searching Spotify…")
                    color: Appearance.colors.onSurfaceVariant
                }
            }
        }

        Item {
            width: root.width
            height: 360
            Reveal.shown: !root.empty && !Library.searchIsCatchphrase && !Library.searching && !Library.hasSearchResults

            EmptyState {
                anchors.centerIn: parent
                symbol: "search_off"
                title: qsTr("No Results for “%1”").arg(Library.searchQuery.trim())
                description: qsTr("Check the spelling or try a new search.")
            }
        }

        // results
        Column {
            id: results
            width: root.width
            Reveal.shown: !root.empty && !Library.searchIsCatchphrase && !Library.searching && Library.hasSearchResults
            spacing: 0

            TitleText {
                x: root.margin
                Reveal.shown: Library.searchSongs.count > 0
                bottomPadding: 10
                text: qsTr("Songs")
                font.pixelSize: Appearance.fontSize.title
                font.weight: Font.Bold
            }

            Repeater {
                model: Library.searchSongs

                SongRow {
                    x: root.margin
                    width: root.width - 2 * root.margin
                    songs: Library.searchSongs
                    showsNumber: false
                    fromSearch: true
                }
            }

            Shelf {
                width: root.width
                onScreen: root.reaches(column.y + results.y + y, height, 0)
                nearScreen: root.reaches(column.y + results.y + y, height, root.height / 2)
                Reveal.shown: Library.searchAlbums.count > 0
                topPadding: 28
                title: qsTr("Albums")
                symbol: "album"
                iconShape: "sunny"
                model: Library.searchAlbums
                delegate: MediaCard {
                    required property album album
                    artworkUrl: album.artworkUrl
                    title: album.name
                    subtitle: album.artists
                    onClicked: Library.selectAlbum(album)
                }
            }

            Shelf {
                width: root.width
                onScreen: root.reaches(column.y + results.y + y, height, 0)
                nearScreen: root.reaches(column.y + results.y + y, height, root.height / 2)
                Reveal.shown: Library.searchArtists.count > 0
                topPadding: 28
                title: qsTr("Artists")
                symbol: "person"
                iconShape: "cookie12"
                model: Library.searchArtists
                delegate: MediaCard {
                    required property artist artist
                    artworkUrl: artist.artworkUrl
                    title: artist.name
                    circular: true
                    titleLines: 1
                    badgeSymbol: "chevron_right"
                    onClicked: Library.selectArtist(artist)
                }
            }

            Shelf {
                width: root.width
                onScreen: root.reaches(column.y + results.y + y, height, 0)
                nearScreen: root.reaches(column.y + results.y + y, height, root.height / 2)
                Reveal.shown: Library.searchPlaylists.count > 0
                topPadding: 28
                title: qsTr("Playlists")
                symbol: "queue_music"
                iconShape: "bun"
                model: Library.searchPlaylists
                delegate: MediaCard {
                    required property playlist playlist
                    artworkUrl: playlist.artworkUrl
                    fallbackKey: playlist.id.length > 0 ? playlist.id : playlist.name
                    title: playlist.name
                    subtitle: playlist.ownerDisplayName
                    onClicked: Library.selectPlaylist(playlist)
                }
            }
        }
    }
}
