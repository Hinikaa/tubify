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

// a titled group of settings on a tonal card, as end4's settings app lays
// them out.
Column {
    id: root

    property string title
    property string symbol
    readonly property list<string> shapes: ["cookie9", "clover4", "sunny", "flower", "bun", "heart", "puffy", "gem"]
    /// the section's own shape, from its title
    property string iconShape: {
        let hash = 0
        for (let index = 0; index < title.length; ++index) {
            hash = (hash * 31 + title.charCodeAt(index)) >>> 0
        }
        return shapes[hash % shapes.length]
    }
    default property alias content: rows.data

    width: parent ? parent.width : 0
    spacing: 8

    Row {
        spacing: 10
        leftPadding: 2

        HoverHandler {
            id: headerHover
        }

        ShapeIcon {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.symbol.length > 0
            size: 34
            iconSize: 18
            shape: root.iconShape
            symbol: root.symbol
            hovered: headerHover.hovered
        }
        TitleText {
            anchors.verticalCenter: parent.verticalCenter
            text: root.title
            font.pixelSize: Appearance.fontSize.larger
        }
    }

    Rectangle {
        width: parent.width
        implicitHeight: rows.implicitHeight + 16
        radius: Appearance.rounding.normal
        color: Appearance.colors.layer1

        Column {
            id: rows
            x: 8
            y: 8
            width: parent.width - 16
            spacing: 2
        }
    }
}
