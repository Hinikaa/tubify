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
import QtQuick.Templates as T
import Swiftify

// a genre to explore: a two-colour gradient with a big faint shape of its
// own cut off by the corner, its symbol on it, turning when hovered.
T.AbstractButton {
    id: root

    property string name
    property string symbol
    property color startColor
    property color endColor

    readonly property list<string> shapes: ["flower", "cookie9", "clover4", "sunny", "puffy", "ghostish", "bun",
                                            "heart", "softBoom", "pixelCircle", "clover8", "gem", "arch", "fan"]
    /// the genre's own shape, from its name
    readonly property string shape: {
        let hash = 0
        for (let index = 0; index < name.length; ++index) {
            hash = (hash * 31 + name.charCodeAt(index)) >>> 0
        }
        return shapes[hash % shapes.length]
    }

    implicitWidth: Appearance.sizes.cardSize
    implicitHeight: 112
    hoverEnabled: true
    // click focus would let other mouse buttons grab and press it
    focusPolicy: Qt.TabFocus
    scale: down ? 0.95 : (hovered ? 1.03 : 1)

    Behavior on scale {
        enabled: !Appearance.performanceMode
        Bounce {}
    }

    background: Item {
        SoftShadow {
            anchors.fill: parent
            radius: Appearance.sizes.artworkRadius
            blur: root.hovered ? 12 : 6
            offset: Qt.vector2d(0, root.hovered ? 6 : 3)
            color: Qt.rgba(0, 0, 0, root.hovered ? 0.2 : 0.1)
        }

        // hovered, its edge turns to running scallops, as a cover's does
        WavyClip {
            anchors.fill: parent
            radius: Appearance.sizes.artworkRadius
            wavy: root.hovered

            Rectangle {
                anchors.fill: parent
                radius: Appearance.sizes.artworkRadius
                gradient: Gradient {
                    orientation: Gradient.Horizontal
                    GradientStop { position: 0; color: root.startColor }
                    GradientStop { position: 1; color: root.endColor }
                }

                MaterialShape {
                    x: parent.width - 100
                    y: -16
                    width: 136
                    height: 136
                    shape: root.shape
                    color: Qt.rgba(1, 1, 1, 0.16)
                    // cut off by the card's edge, round corner and all
                    parentClipRadius: Appearance.sizes.artworkRadius
                    spinAngle: root.hovered ? 34 : 8

                    Behavior on spinAngle {
                        enabled: !Appearance.performanceMode
                        Bounce {}
                    }

                    MaterialSymbol {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: -8
                        anchors.verticalCenterOffset: 8
                        text: root.symbol
                        iconSize: 50
                        fill: 1
                        weight: 600
                        rotation: 10
                        color: Qt.rgba(1, 1, 1, 0.34)
                    }
                }
            }
        }
    }

    contentItem: Item {
        StyledText {
            anchors.left: parent.left
            anchors.bottom: parent.bottom
            anchors.margins: 14
                text: qsTranslate("Genres", root.name)
            color: "white"
            font.pixelSize: Appearance.fontSize.larger
            font.weight: Font.Bold
        }
    }

    // leans a few degrees toward the pointer while hovered, on a spring
    property real tiltAcross: pointer.hovered && !Appearance.performanceMode
                              ? (pointer.point.position.x / Math.max(1, width) - 0.5) * 12 : 0
    property real tiltDown: pointer.hovered && !Appearance.performanceMode
                            ? (0.5 - pointer.point.position.y / Math.max(1, root.height)) * 10 : 0

    Behavior on tiltAcross {
        SpringAnimation { spring: 4; damping: 0.3; epsilon: 0.01 }
    }
    Behavior on tiltDown {
        SpringAnimation { spring: 4; damping: 0.3; epsilon: 0.01 }
    }

    transform: [
        Rotation {
            origin.x: root.width / 2
            origin.y: root.height / 2
            axis { x: 0; y: 1; z: 0 }
            angle: root.tiltAcross
        },
        Rotation {
            origin.x: root.width / 2
            origin.y: root.height / 2
            axis { x: 1; y: 0; z: 0 }
            angle: root.tiltDown
        }
    ]

    HoverHandler {
        id: pointer
        cursorShape: Qt.PointingHandCursor
    }
}
