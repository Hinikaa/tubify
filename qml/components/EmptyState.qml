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
import Swiftify

// what a page shows with nothing in it: end4's page placeholder, a symbol on a
// tonal cookie over a title and a line of explanation. the cookie springs in
// with a twist each time the placeholder appears.
Column {
    id: root

    property string symbol: "music_note"
    /// the badge's shape, picked for what the placeholder says
    property string shape: {
        switch (symbol) {
        case "search_off": return "ghostish"
        case "music_note": return "heart"
        case "lyrics": return "bun"
        case "queue_music": return "clamShell"
        case "push_pin": return "clover4"
        case "album": return "sunny"
        case "star": return "heart"
        case "cloud_off":
        case "wifi_off": return "pixelCircle"
        default: return "cookie9"
        }
    }
    property string title
    property string description
    property MaterialScheme scheme: Appearance.colors

    spacing: 10
    width: Math.min(parent ? parent.width : 400, 420)

    onVisibleChanged: if (visible && !Appearance.performanceMode) arrive.restart()

    MaterialShape {
        id: badge
        anchors.horizontalCenter: parent.horizontalCenter
        width: 80
        height: 80
        shape: root.shape
        color: root.scheme.secondaryContainer

        // little shapes about the badge, behind it
        Stickers {
            anchors.centerIn: parent
            width: 190
            height: 120
            z: -1
            key: root.title
            count: 3
            minimumSize: 16
            maximumSize: 30
            strength: 1.8
            scheme: root.scheme
        }

        ParallelAnimation {
            id: arrive
            NumberAnimation { target: badge; property: "scale"; from: 0.4; to: 1; duration: 700; easing.type: Easing.OutBack; easing.overshoot: 2.2 }
            NumberAnimation { target: badge; property: "rotation"; from: -60; to: 0; duration: 900; easing.type: Easing.OutBack; easing.overshoot: 1.4 }
        }

        MaterialSymbol {
            anchors.centerIn: parent
            text: root.symbol
            iconSize: 34
            fill: 1
            color: root.scheme.onSecondaryContainer
        }
    }

    TitleText {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: root.title
        font.pixelSize: Appearance.fontSize.larger
        color: root.scheme.onSurface
        wrapMode: Text.Wrap
    }

    StyledText {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        visible: root.description.length > 0
        text: root.description
        color: root.scheme.onSurfaceVariant
        wrapMode: Text.Wrap
    }
}
