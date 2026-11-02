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

// one figure of the overview: a shape with its icon, the figure, and what it
// counts (listening-stats' overview cards)
Rectangle {
    id: root

    property string value
    property string label
    property string symbol
    property string iconShape: "cookie9"
    property bool accent: false

    implicitHeight: 96
    radius: 20
    color: accent ? Appearance.colors.primaryContainer : Appearance.colors.surfaceContainerLow
    border.width: 0.75
    border.color: Qt.rgba(1, 1, 1, accent ? 0 : 0.07)

    ShapeIcon {
        id: icon
        x: 16
        anchors.verticalCenter: parent.verticalCenter
        size: 44
        iconSize: 22
        shape: root.iconShape
        symbol: root.symbol
        hovered: hover.hovered
        color: root.accent ? Appearance.colors.primary : Appearance.colors.secondaryContainer
        iconColor: root.accent ? Appearance.colors.onPrimary : Appearance.colors.onSecondaryContainer
    }

    Column {
        anchors.left: icon.right
        anchors.leftMargin: 14
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        TitleText {
            width: parent.width
            text: root.value
            maximumLineCount: 1
            wrapMode: Text.NoWrap
            elide: Text.ElideRight
            font.pixelSize: Appearance.fontSize.huge
            font.weight: Font.Bold
            color: root.accent ? Appearance.colors.onPrimaryContainer : Appearance.colors.onSurface
        }
        StyledText {
            width: parent.width
            text: root.label
            elide: Text.ElideRight
            font.pixelSize: Appearance.fontSize.smaller
            color: root.accent ? Appearance.colors.onPrimaryContainer : Appearance.colors.onSurfaceVariant
        }
    }

    HoverHandler {
        id: hover
    }
}
