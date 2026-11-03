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
import QtQuick.Controls.Basic
import Tubify

// connecting YouTube Music for the first time
Flickable {
    id: root

    // pages run up under the top bar, which floats over them with no
    // background of its own; they start below it
    topMargin: Appearance.sizes.topBarHeight

    contentHeight: Math.max(height, column.implicitHeight + 72)
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: StyledScrollBar { topPadding: 3 + Appearance.sizes.topBarHeight }

    component Step: Row {
        id: step
        property int number
        property string title
        property string detail

        spacing: 12
        width: parent ? parent.width : 560

        Rectangle {
            width: 26
            height: 26
            radius: 13
            color: Appearance.colors.primary

            StyledText {
                anchors.centerIn: parent
                text: step.number
                font.weight: Font.Bold
                font.pixelSize: Appearance.fontSize.smallie
                color: Appearance.colors.onPrimary
            }
        }

        Column {
            width: step.width - 38
            spacing: 2

            StyledText {
                text: step.title
                font.pixelSize: Appearance.fontSize.normal
                font.weight: Font.DemiBold
            }
            StyledText {
                width: parent.width
                text: step.detail
                wrapMode: Text.Wrap
                color: Appearance.colors.onSurfaceVariant
                font.pixelSize: Appearance.fontSize.smallie
            }
        }
    }

    Stickers {
        width: root.width
        height: Math.max(root.height, column.y + column.height)
        key: "setup"
        count: 6
    }

    Column {
        id: column
        anchors.horizontalCenter: parent.horizontalCenter
        y: Math.max(36, (root.height - implicitHeight) / 2)
        width: Math.min(560, root.width - 72)
        spacing: 20

        MaterialSymbol {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 64
            height: 64
            text: "library_music"
            iconSize: 56
            fill: 1
            color: Appearance.colors.primary
        }

        TitleText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: qsTr("Connect YouTube Music")
            font.pixelSize: Appearance.fontSize.display
            font.weight: Font.Bold
        }

        StyledText {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: qsTr("Tubify signs in with the Cookie header from your browser. One sign-in opens your library and playback together, and you only have to do it once.")
            color: Appearance.colors.onSurfaceVariant
        }

        Column {
            width: parent.width
            spacing: 14

            Step {
                number: 1
                title: qsTr("Open YouTube Music")
                detail: qsTr("Sign in at music.youtube.com in your browser.")
            }

            TextButton {
                x: 38
                variant: TextButton.Text
                symbol: "open_in_new"
                text: qsTr("Open music.youtube.com")
                onClicked: App.openUrl("https://music.youtube.com")
            }

            Step {
                number: 2
                title: qsTr("Copy your cookie")
                detail: qsTr("Press F12, open the Network tab, reload the page, click any request to music.youtube.com, and copy the whole Cookie request header.")
            }

            Step {
                number: 3
                title: qsTr("Paste it below")
                detail: qsTr("The cookie stays on this computer, in a file only you can read.")
            }
        }

        StyledText {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: qsTr("or read it straight from your browser")
            color: Appearance.colors.onSurfaceVariant
            font.pixelSize: Appearance.fontSize.smaller
        }

        Flow {
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(460, parent.width)
            spacing: 8

            TextButton { text: qsTr("Helium"); symbol: "travel_explore"; enabled: !Session.loginInProgress; onClicked: Session.signInWithBrowser("helium") }
            TextButton { text: qsTr("Chromium"); symbol: "travel_explore"; enabled: !Session.loginInProgress; onClicked: Session.signInWithBrowser("chromium") }
            TextButton { text: qsTr("Chrome"); symbol: "travel_explore"; enabled: !Session.loginInProgress; onClicked: Session.signInWithBrowser("chrome") }
            TextButton { text: qsTr("Brave"); symbol: "travel_explore"; enabled: !Session.loginInProgress; onClicked: Session.signInWithBrowser("brave") }
            TextButton { text: qsTr("Firefox"); symbol: "travel_explore"; enabled: !Session.loginInProgress; onClicked: Session.signInWithBrowser("firefox") }
        }

        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(460, parent.width)
            height: 132
            radius: Appearance.rounding.small
            color: Appearance.colors.surfaceContainerHigh
            border.width: 1
            border.color: cookieArea.activeFocus ? Appearance.colors.primary : Appearance.colors.outlineVariant

            TextArea {
                id: cookieArea
                anchors.fill: parent
                anchors.margins: 10
                placeholderText: qsTr("Cookie: SAPISID=…; __Secure-3PAPISID=…; SID=…")
                wrapMode: TextEdit.WrapAnywhere
                selectByMouse: true
                font.family: Appearance.monoFontFamily
                font.pixelSize: Appearance.fontSize.smallie
                color: Appearance.colors.onSurface
                placeholderTextColor: Appearance.colors.onSurfaceVariant
                selectionColor: Appearance.colors.primary
                selectedTextColor: Appearance.colors.onPrimary
                background: null
            }
        }

        TextButton {
            anchors.horizontalCenter: parent.horizontalCenter
            variant: Session.connected ? TextButton.Tonal : TextButton.Filled
            symbol: Session.connected ? "check" : (Session.loginInProgress ? "progress_activity" : "login")
            text: Session.connected ? qsTr("Connected")
                : Session.loginInProgress ? qsTr("Signing In…")
                : qsTr("Sign In")
            enabled: !Session.connected && !Session.loginInProgress && cookieArea.text.trim().length > 0
            onClicked: Session.signIn(cookieArea.text)
        }

        StyledText {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: Session.connected
                  ? qsTr("Connected. Your library is loading…")
                  : qsTr("Paste the Cookie header above and choose Sign In.")
            font.pixelSize: Appearance.fontSize.smaller
            color: Session.connected ? Appearance.colors.primary : Appearance.colors.onSurfaceVariant
        }
    }
}
