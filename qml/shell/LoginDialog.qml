/*
 * Tubify, a YouTube Music player for desktop
 * Copyright (C) 2026  Hinika
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

// the one sign-in YouTube Music needs: the Cookie header from a signed-in
// music.youtube.com. it opens the library and playback at once.
StyledDialog {
    id: root

    readonly property bool signingIn: Session.loginInProgress

    headline: qsTr("Sign in to YouTube Music")
    symbol: "key"
    iconShape: "clover4"
    body: qsTr("Open music.youtube.com in your browser and sign in. Press F12, open the Network tab, reload the page, click any request to music.youtube.com, and copy the whole Cookie request header. Paste it below.")

    Connections {
        target: Session
        function onLoginRequested() {
            root.open()
        }
        function onLoginChanged() {
            if (Session.connected && root.opened) {
                root.accept()
            }
        }
    }

    onOpened: cookieArea.forceActiveFocus()

    Flow {
        width: parent.width
        spacing: 8

        TextButton {
            text: qsTr("Helium")
            symbol: "travel_explore"
            enabled: !root.signingIn
            onClicked: Session.signInWithBrowser("helium")
        }
        TextButton {
            text: qsTr("Chromium")
            symbol: "travel_explore"
            enabled: !root.signingIn
            onClicked: Session.signInWithBrowser("chromium")
        }
        TextButton {
            text: qsTr("Chrome")
            symbol: "travel_explore"
            enabled: !root.signingIn
            onClicked: Session.signInWithBrowser("chrome")
        }
        TextButton {
            text: qsTr("Brave")
            symbol: "travel_explore"
            enabled: !root.signingIn
            onClicked: Session.signInWithBrowser("brave")
        }
        TextButton {
            text: qsTr("Firefox")
            symbol: "travel_explore"
            enabled: !root.signingIn
            onClicked: Session.signInWithBrowser("firefox")
        }
    }

    StyledText {
        width: parent.width
        text: qsTr("reads the cookie from that browser's profile. close the browser first if it is running. or paste it below.")
        wrapMode: Text.Wrap
        font.pixelSize: Appearance.fontSize.smaller
        color: Appearance.colors.onSurfaceVariant
    }

    Rectangle {
        width: parent.width
        height: 150
        radius: Appearance.rounding.small
        color: Appearance.colors.surfaceContainer
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

    Row {
        spacing: 8

        MaterialSymbol {
            anchors.verticalCenter: parent.verticalCenter
            width: 20
            height: 20
            text: root.signingIn ? "progress_activity" : "info"
            iconSize: 18
            color: root.signingIn ? Appearance.colors.primary : Appearance.colors.onSurfaceVariant
        }
        StyledText {
            anchors.verticalCenter: parent.verticalCenter
            width: root.width - 36
            wrapMode: Text.Wrap
            text: root.signingIn
                  ? qsTr("Checking the cookie with YouTube Music…")
                  : qsTr("The cookie stays on this computer, in a file only you can read.")
            color: Appearance.colors.onSurfaceVariant
            font.pixelSize: Appearance.fontSize.smallie
        }
    }

    buttons: [
        TextButton {
            variant: TextButton.Text
            text: qsTr("Cancel")
            onClicked: {
                Session.cancelLogin()
                root.close()
            }
        },
        TextButton {
            variant: TextButton.Filled
            symbol: "login"
            text: root.signingIn ? qsTr("Signing In…") : qsTr("Sign In")
            enabled: !root.signingIn && cookieArea.text.trim().length > 0
            onClicked: Session.signIn(cookieArea.text)
        }
    ]
}
