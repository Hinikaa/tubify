/*
 * Tubify, a YouTube Music player for desktop
 * Copyright (C) 2026  Hinikaa
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
import Tubify

// minimise, maximise and close, drawn as the three traffic lights macOS puts
// in a window's corner: a red close, a yellow minimise, a green zoom.
Row {
    id: root

    required property Window window
    property bool compact: false
    property MaterialScheme scheme: Appearance.colors
    layoutDirection: Settings.windowControlsOnRight ? Qt.RightToLeft : Qt.LeftToRight

    spacing: root.compact ? 8 : 10

    component TrafficLight: Item {
        id: light

        required property color baseColor
        required property color hoverColor
        required property color glyphColor
        required property string glyph
        required property string tooltip
        signal clicked()

        width: root.compact ? 18 : 20
        height: width

        Rectangle {
            anchors.centerIn: parent
            width: root.compact ? 13 : 14
            height: width
            radius: width / 2
            color: pointer.containsMouse ? light.hoverColor : light.baseColor
            border.width: pointer.containsMouse ? 0 : 1
            border.color: Qt.rgba(0, 0, 0, 0.14)
            Behavior on color { ColorAnimation { duration: 120 } }

            MaterialSymbol {
                anchors.centerIn: parent
                width: parent.width
                height: parent.height
                text: light.glyph
                iconSize: root.compact ? 8 : 9
                color: light.glyphColor
                opacity: pointer.containsMouse ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 100 } }
            }
        }

        MouseArea {
            id: pointer
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: light.clicked()
        }
    }

    TrafficLight {
        objectName: "closeWindowButton"
        baseColor: "#ff5f57"
        hoverColor: "#ff3b30"
        glyphColor: "#7a0000"
        glyph: "close"
        tooltip: qsTr("Close window")
        onClicked: root.window.close()
    }
    TrafficLight {
        objectName: "minimizeWindowButton"
        baseColor: "#febc2e"
        hoverColor: "#ffb01f"
        glyphColor: "#8a5a00"
        glyph: "remove"
        tooltip: qsTr("Minimize")
        onClicked: root.window.showMinimized()
    }
    TrafficLight {
        objectName: "maximizeWindowButton"
        baseColor: "#28c840"
        hoverColor: "#1fbf34"
        glyphColor: "#006400"
        glyph: root.window.visibility === Window.Maximized ? "close_fullscreen" : "open_in_full"
        tooltip: root.window.visibility === Window.Maximized ? qsTr("Restore") : qsTr("Maximize")
        onClicked: root.window.visibility === Window.Maximized ? root.window.showNormal() : root.window.showMaximized()
    }
}
