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

// a setting that is one of a few choices: its name, then the choices as
// segmented buttons (end4's ConfigSelectionArray).
Item {
    id: root

    property string text
    property alias segments: choices.segments
    property alias currentIndex: choices.currentIndex
    signal activated(int index)

    width: parent ? parent.width : 0
    implicitHeight: Math.max(52, choices.implicitHeight + 12)
    opacity: enabled ? 1 : 0.38

    StyledText {
        x: 12
        anchors.right: choices.left
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        text: root.text
        elide: Text.ElideRight
    }

    SegmentedControl {
        id: choices
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        segmentHeight: 32
        enabled: root.enabled
        onActivated: (index) => root.activated(index)
    }
}
