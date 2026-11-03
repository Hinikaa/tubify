#!/usr/bin/env bash
# Swiftify, a spotify player for desktop
# Copyright (C) 2026  shred0s
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

# builds an arm64 Tubify.app with qt bundled, signed ad hoc, in a dmg,
# against a qt kit from the qt installer or aqtinstall
#
# usage
#   ./tools/build-macos-dmg.sh --qt ~/Qt/6.12.0/macos
#   ./tools/build-macos-dmg.sh --qt ~/Qt/6.12.0/macos --out dist --jobs 8
#   ./tools/build-macos-dmg.sh --qt ~/Qt/6.12.0/macos \
#       --sign "Developer ID Application: <name> (<team>)" --notary-profile tubify
#
# needs an apple silicon mac, cmake, ninja, git, xcode or its command line
# tools, and rust from rustup. with full xcode the icon is compiled with actool
#
# --sign signs with the hardened runtime and packaging/macos/Tubify.entitlements,
# which is what notarising needs, and --notary-profile notarises the dmg with
# the keychain profile xcrun notarytool store-credentials saved, then staples
# the ticket to it. both are off unless asked for
#
# without either, the dmg is signed ad hoc, and a copy that came off the
# network carries the quarantine flag: the first launch needs it cleared
# (xattr -dr com.apple.quarantine /Applications/Tubify.app) or a right click
# and open

set -euo pipefail

QT=""
OUT="dist"
JOBS="$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
IDENTITY="-"
NOTARY_PROFILE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --qt) QT="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --jobs) JOBS="$2"; shift 2 ;;
        --sign) IDENTITY="$2"; shift 2 ;;
        --notary-profile) NOTARY_PROFILE="$2"; shift 2 ;;
        -h|--help)
            sed -n '18,38p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown option $1" >&2; exit 1 ;;
    esac
done

fail() { echo "error, $1" >&2; exit 1; }

[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || fail "this builds on apple silicon only"
[[ -n "$QT" ]] || fail "pass --qt with the path of a qt 6.10 or newer macos kit"
QT="$(cd "$QT" && pwd)"
[[ -x "$QT/bin/macdeployqt" ]] || fail "no macdeployqt in $QT/bin, pass the macos folder of the kit"
for tool in cmake ninja git cargo codesign hdiutil lipo vtool; do
    command -v "$tool" >/dev/null || fail "$tool is missing"
done

SOURCE="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(sed -n '/^project(Tubify/,/VERSION/s/^ *VERSION \([0-9.]*\).*/\1/p' "$SOURCE/CMakeLists.txt")"
[[ -n "$VERSION" ]] || fail "could not read the version from CMakeLists.txt"
NAME="tubify-$VERSION-macos-arm64"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
WORK="$SOURCE/build-macos-dmg"
DEPS="$WORK/deps"
APP="$WORK/Tubify.app"
mkdir -p "$WORK/sources"

# the oldest macos the kit's qt runs on, which the app can't go below
MINIMUM="$(vtool -arch arm64 -show-build "$QT/lib/QtCore.framework/QtCore" | awk '/minos/ { print $2; exit }')"
[[ -n "$MINIMUM" ]] || fail "could not read the oldest macos qt runs on"
export MACOSX_DEPLOYMENT_TARGET="$MINIMUM"

echo "qt $QT"
echo "version $VERSION"
echo "macos $MINIMUM or newer"
echo "output $OUT/$NAME.dmg"

# homebrew's qt, qtkeychain and qcoro are left out, so only the kit's qt and
# the libraries built here get linked
common=(-G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF
    "-DCMAKE_PREFIX_PATH=$QT;$DEPS" "-DCMAKE_IGNORE_PREFIX_PATH=/opt/homebrew;/usr/local"
    -DCMAKE_OSX_ARCHITECTURES=arm64 "-DCMAKE_OSX_DEPLOYMENT_TARGET=$MINIMUM")

# qtkeychain and qcoro are built static against the same qt, at tags known to
# work, so there is nothing of theirs to bundle
build_dependency() {
    local name="$1" url="$2" tag="$3"
    shift 3
    if [[ ! -d "$WORK/sources/$name" ]]; then
        git clone --quiet --depth 1 --branch "$tag" "$url" "$WORK/sources/$name"
    fi
    cmake -S "$WORK/sources/$name" -B "$WORK/$name" "${common[@]}" -DBUILD_SHARED_LIBS=OFF \
        "-DCMAKE_INSTALL_PREFIX=$DEPS" -DCMAKE_INSTALL_LIBDIR=lib "$@"
    cmake --build "$WORK/$name" --parallel "$JOBS"
    cmake --install "$WORK/$name"
}
build_dependency qtkeychain https://github.com/frankosterfeld/qtkeychain.git 0.17.0 \
    -DBUILD_WITH_QT5=OFF -DBUILD_WITH_QT6=ON -DBUILD_TEST_APPLICATION=OFF -DBUILD_TRANSLATIONS=OFF
build_dependency qcoro https://github.com/qcoro/qcoro.git v0.13.0 \
    -DUSE_QT_VERSION=6 -DQCORO_WITH_QTDBUS=OFF -DQCORO_WITH_QTWEBSOCKETS=OFF -DQCORO_WITH_QTQUICK=OFF \
    -DQCORO_WITH_QML=OFF -DQCORO_WITH_QTTEST=OFF -DQCORO_BUILD_EXAMPLES=OFF -DBUILD_TESTING=OFF

cmake -S "$SOURCE" -B "$WORK/swiftify" "${common[@]}" -DRust_CARGO_TARGET=aarch64-apple-darwin -DSWIFTIFY_BUNDLED_QT=ON
cmake --build "$WORK/swiftify" --parallel "$JOBS"

rm -rf "${APP:?}"
ditto "$WORK/swiftify/Tubify.app" "$APP"
plutil -replace LSMinimumSystemVersion -string "$MINIMUM" "$APP/Contents/Info.plist"
"$QT/bin/macdeployqt" "$APP" "-qmldir=$SOURCE/qml"

# qml's local storage brings qt's sql drivers along. all but sqlite link
# database clients from outside the app, and nothing uses them
for driver in odbc psql mimer; do
    rm -f "$APP/Contents/PlugIns/sqldrivers/libqsql$driver.dylib"
done

# the kit's qt is universal, the app keeps only its arm64 half
while IFS= read -r -d '' file; do
    if file -b "$file" | grep -q "universal binary"; then
        lipo "$file" -thin arm64 -output "$file.arm64"
        mv "$file.arm64" "$file"
    fi
done < <(find "$APP" -type f -print0)

# macdeployqt rewrote what the build signed, so the whole bundle is signed
# again. ad hoc unless an identity is given, which is also what lets it be
# notarised: the hardened runtime and the entitlements notarising wants go
# with it, and a secure timestamp, which an ad hoc signature cannot carry
xattr -cr "$APP"
if [[ "$IDENTITY" == "-" ]]; then
    codesign --force --deep --sign - "$APP"
else
    codesign --force --deep --sign "$IDENTITY" --options runtime --timestamp \
        --entitlements "$SOURCE/packaging/macos/Tubify.entitlements" "$APP"
fi
codesign --verify --deep --strict "$APP" || fail "the app's signature does not verify"

# everything in it has to be arm64 alone, and find its libraries in the app
# or the system, with none of the build machine's paths
problems=""
while IFS= read -r -d '' file; do
    file -b "$file" | grep -q Mach-O || continue
    relative="${file#"$APP/"}"
    archs="$(lipo -archs "$file")"
    [[ "$archs" == arm64 ]] || problems+="$relative is $archs"$'\n'
    while read -r library; do
        case $library in
            @rpath/*|@executable_path/*|@loader_path/*|/System/*|/usr/lib/*) ;;
            *) problems+="$relative links $library"$'\n' ;;
        esac
    done < <(otool -L "$file" | tail -n +2 | awk '{ print $1 }')
done < <(find "$APP" -type f -print0)
[[ -z "$problems" ]] || fail "the app is not self contained"$'\n'"$problems"
[[ -d "$APP/Contents/PlugIns/tls" ]] || fail "the tls plugins are missing"
[[ -f "$APP/Contents/PlugIns/platforms/libqcocoa.dylib" ]] || fail "the cocoa platform plugin is missing"
[[ -d "$APP/Contents/Resources/qml/QtQuick" ]] || fail "qt's qml modules are missing"
[[ -f "$APP/Contents/Resources/LICENSE" ]] || fail "the license is missing"
for license in material-color-utilities.LICENSE rounded-polygon-qmljs.LICENSE material-symbols.LICENSE \
    rust-crates.LICENSE qt.LICENSE qt.NOTICE qtkeychain.LICENSE qcoro.LICENSE; do
    [[ -f "$APP/Contents/Resources/$license" ]] || fail "$license is missing"
done

# the dmg opens to the app beside a link to applications. hdiutil sometimes
# finds the volume busy on ci, so it gets a few tries
STAGE="$WORK/dmg"
rm -rf "${STAGE:?}"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Tubify.app"
ln -s /Applications "$STAGE/Applications"
rm -f "$OUT/$NAME.dmg"
for attempt in 1 2 3; do
    if hdiutil create -volname Tubify -srcfolder "$STAGE" -fs HFS+ -format UDZO "$OUT/$NAME.dmg"; then
        break
    fi
    [[ $attempt -lt 3 ]] || fail "hdiutil could not make the dmg"
    sleep 5
done
hdiutil verify "$OUT/$NAME.dmg" >/dev/null || fail "the dmg does not verify"

# notarised with the keychain profile given, and the ticket stapled to the
# dmg, so a mac that downloads it opens it without the quarantine warning
if [[ -n "$NOTARY_PROFILE" ]]; then
    xcrun notarytool submit "$OUT/$NAME.dmg" --keychain-profile "$NOTARY_PROFILE" --wait \
        || fail "notarytool refused the dmg"
    xcrun stapler staple "$OUT/$NAME.dmg" || fail "the ticket could not be stapled to the dmg"
    xcrun stapler validate "$OUT/$NAME.dmg" || fail "the dmg carries no ticket"
fi
echo "built $OUT/$NAME.dmg"
