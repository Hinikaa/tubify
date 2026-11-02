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

// a titled horizontal row of cards (ResultShelf). it scrolls under the page
// margins to the window's edges.
Column {
    id: root

    property string title
    /// the symbol and shape its heading wears
    property string symbol
    property string iconShape: "cookie9"
    property bool showsSeeAll: false
    property var model
    property Component delegate
    property real pageMargin: Appearance.sizes.pageMargin
    property real rowHeight: Appearance.sizes.cardSize + 64
    /// whether any of it is in sight on its page (the page's reaches()). out
    /// of it, it keeps its place but draws nothing: every frame the window
    /// drew went through each card of every shelf down the page
    property bool onScreen: true
    /// whether it is in sight or nearly, half a screen off: its cards are
    /// built ahead, so a scroll never shows a shelf arriving. further off
    /// they are let go of, as a card hidden rather than gone kept its cover
    /// decoded in memory, never drawn and so never handed to the gpu
    property bool nearScreen: onScreen
    /// where the row was scrolled to, kept while its cards are gone
    property real scrolled: 0

    signal seeAll()

    spacing: 13

    // the heading and the row are hidden inside holders that keep their
    // size, so the page's layout is the same either way
    Item {
        width: root.width
        height: header.implicitHeight

        SectionHeader {
            id: header
            x: root.pageMargin
            width: root.width - 2 * root.pageMargin
            visible: root.onScreen
            title: root.title
            symbol: root.symbol
            iconShape: root.iconShape
            showsSeeAll: root.showsSeeAll
            onSeeAll: root.seeAll()
        }
    }

    Item {
        width: root.width
        height: root.rowHeight

        // built over a few frames as it nears the screen, rather than in the
        // middle of one
        Loader {
            anchors.fill: parent
            active: root.nearScreen
            visible: root.onScreen
            asynchronous: true
            sourceComponent: ListView {
                orientation: ListView.Horizontal
                spacing: Appearance.sizes.shelfGap
                leftMargin: root.pageMargin
                rightMargin: root.pageMargin
                boundsBehavior: Flickable.StopAtBounds
                reuseItems: true
                // no cards built ahead off the edge: each held its cover
                // decoded in memory, never drawn, for as long as the page was kept
                cacheBuffer: 0
                model: root.model
                delegate: root.delegate
                ScrollBar.horizontal: StyledScrollBar {}

                // back where it was; what it reports before then is its start
                property bool restored: false
                Component.onCompleted: {
                    contentX = root.scrolled - leftMargin
                    restored = true
                }
                onContentXChanged: {
                    if (restored) {
                        root.scrolled = contentX + leftMargin
                    }
                }
            }
        }
    }
}
