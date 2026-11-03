#[[
    Swiftify, a spotify player for desktop
    Copyright (C) 2026  shred0s

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
]]

cmake_minimum_required(VERSION 3.24)

# the oldest macos the app runs on. it is what the compiler, the bundle's
# LSMinimumSystemVersion and the frameworks it carries are built against, and
# it has to be set before the compiler is first asked what it can do. 13 is
# the oldest macos apple still updates, and what qt 6.10 needs at least
if(APPLE AND NOT CMAKE_OSX_DEPLOYMENT_TARGET)
    set(CMAKE_OSX_DEPLOYMENT_TARGET "13.0" CACHE STRING
        "The oldest macOS the app runs on, which the bundle and Qt are built against")
endif()

project(Tubify
    VERSION 0.1.0
    DESCRIPTION "A native YouTube Music client"
    LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# the window's title bar on macos is styled through appkit
if(APPLE)
    enable_language(OBJCXX)
    set(CMAKE_OBJCXX_STANDARD 20)
    set(CMAKE_OBJCXX_STANDARD_REQUIRED ON)
endif()

# on windows <cmath> gives M_PI only when asked, and windows.h's min and
# max macros are kept off std::min and std::max; msvc also reads the
# sources as utf-8
if(WIN32)
    add_compile_definitions(_USE_MATH_DEFINES NOMINMAX)
endif()
if(MSVC)
    add_compile_options(/utf-8)
endif()

include(GNUInstallDirs)
include(CTest)

if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    set(SWIFTIFY_IS_LINUX ON)
else()
    set(SWIFTIFY_IS_LINUX OFF)
endif()

# the wayland, d-bus and pipewire pieces. off elsewhere so the portable parts
# can be built and tested on a development machine without them.
option(SWIFTIFY_DESKTOP_INTEGRATION "Build the Linux desktop integration" ${SWIFTIFY_IS_LINUX})
# spotify soloist plays spotify's songs for accounts spotify no longer gives
# librespot the keys for.
option(SWIFTIFY_SOLOIST "Play Spotify songs through Spotify Soloist when it is installed" OFF)

find_package(Qt6 6.10 REQUIRED COMPONENTS
    Core Gui Network Concurrent Qml Quick QuickControls2 ShaderTools)
if(BUILD_TESTING)
    find_package(Qt6 6.10 REQUIRED COMPONENTS Test QuickTest)
endif()
if(SWIFTIFY_IS_LINUX)
    find_package(Qt6 6.10 REQUIRED COMPONENTS DBus Widgets)
else()
    find_package(Qt6 6.10 OPTIONAL_COMPONENTS DBus)
endif()
# mpris and logind are linux's: qt ships d-bus on macos too, but nothing here
# talks to it there, and linking it would drag libdbus into the bundle
if(SWIFTIFY_IS_LINUX AND TARGET Qt6::DBus)
    set(SWIFTIFY_DBUS ON)
else()
    set(SWIFTIFY_DBUS OFF)
endif()
if(SWIFTIFY_DESKTOP_INTEGRATION)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(PULSE REQUIRED IMPORTED_TARGET libpulse)
endif()
if(WIN32)
    find_package(Qt6 6.10 REQUIRED COMPONENTS Widgets)
endif()
find_package(QCoro6 0.10 QUIET COMPONENTS Core Network)
if(NOT QCoro6_FOUND)
    include(FetchContent)
    FetchContent_Declare(qcoro
        GIT_REPOSITORY https://github.com/qcoro/qcoro.git
        GIT_TAG v0.13.0)
    set(QCORO_BUILD_TESTING OFF CACHE BOOL "" FORCE)
    set(QCORO_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(QCORO_WITH_QTWEBSOCKETS OFF CACHE BOOL "" FORCE)
    set(QCORO_WITH_QML OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(qcoro)
endif()
qcoro_enable_coroutines()

qt_standard_project_setup(REQUIRES 6.10)
# modules get their own folder so tooling finds them as <dir>/Tubify/qmldir,
# clear of the executable (the same name on a case-insensitive disk).
set(QT_QML_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/qml_modules)

# --- ytmusic-core (Rust) -----------------------------------------------------

# the core sits inside this folder in swiftify-linux, and beside it in the
# swiftify repository with the apple apps
if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/ytmusic-core/Cargo.toml)
    set(SWIFTIFY_CORE_DEFAULT ${CMAKE_CURRENT_SOURCE_DIR}/ytmusic-core)
else()
    set(SWIFTIFY_CORE_DEFAULT ${CMAKE_CURRENT_SOURCE_DIR}/../ytmusic-core)
endif()
set(SWIFTIFY_CORE_DIR ${SWIFTIFY_CORE_DEFAULT} CACHE PATH "The Rust core, ytmusic-core")

include(FetchContent)
FetchContent_Declare(Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6.1)
FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(
    MANIFEST_PATH ${SWIFTIFY_CORE_DIR}/Cargo.toml
    CRATE_TYPES staticlib
    NO_DEFAULT_FEATURES
    FEATURES capi)
target_include_directories(ytmusic_core INTERFACE
    ${SWIFTIFY_CORE_DIR}/include)
# what rustc reports as the staticlib's native dependencies
# (cargo rustc -- --print native-static-libs).
if(APPLE)
    # what cpal plays through on macos: coreaudio's auhal output unit lives in
    # AudioToolbox and AudioUnit, and rust's std and rustls bring the rest.
    # audio_unit is not in coreaudio's older lists, and its symbols are only
    # reached at run time, so leaving it out links and then fails to open a
    # device
    target_link_libraries(ytmusic_core INTERFACE
        "-framework AudioToolbox" "-framework AudioUnit" "-framework CoreAudio"
        "-framework IOKit" "-framework CoreFoundation" "-framework Foundation"
        "-framework SystemConfiguration" "-framework Security" objc iconv)
elseif(WIN32)
    # windows: wasapi for audio, schannel for tls, and what rust's std
    # needs; the linker leaves out any of them nothing uses
    target_link_libraries(ytmusic_core INTERFACE
        ws2_32 userenv bcrypt ntdll advapi32 kernel32 user32 shell32 ole32 oleaut32
        propsys crypt32 secur32 ncrypt runtimeobject synchronization)
else()
    find_package(ALSA REQUIRED)
    find_package(OpenSSL REQUIRED)
    find_package(Threads REQUIRED)
    target_link_libraries(ytmusic_core INTERFACE
        ALSA::ALSA OpenSSL::SSL OpenSSL::Crypto Threads::Threads ${CMAKE_DL_LIBS} m)
endif()

# --- material-color-utilities ------------------------------------------------

file(GLOB_RECURSE MCU_SOURCES CONFIGURE_DEPENDS
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/material-color-utilities/cpp/*.cc)
add_library(material_color_utilities STATIC ${MCU_SOURCES})
target_include_directories(material_color_utilities PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/material-color-utilities)
set_target_properties(material_color_utilities PROPERTIES POSITION_INDEPENDENT_CODE ON)

# --- fonts ---------------------------------------------------------------------

option(SWIFTIFY_DOWNLOAD_ICON_FONT
    "Download Material Symbols Rounded to install beside the app" ON)
include(cmake/DownloadAsset.cmake)
set(SWIFTIFY_FONT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/fonts)
if(SWIFTIFY_DOWNLOAD_ICON_FONT)
    set(SWIFTIFY_ICON_FONT ${SWIFTIFY_FONT_DIRECTORY}/MaterialSymbolsRounded.ttf)
    swiftify_download_asset(
        "https://raw.githubusercontent.com/google/material-design-icons/737e3324305806514d7909874fa1818ae1808232/variablefont/MaterialSymbolsRounded%5BFILL,GRAD,opsz,wght%5D.ttf"
        "${SWIFTIFY_ICON_FONT}"
        95b24392bb49efd1bc3e92cff4e2452ad094461bab7c97e7d8723fab97e330ca)
    if(NOT APPLE)
        install(FILES ${SWIFTIFY_ICON_FONT} DESTINATION ${CMAKE_INSTALL_DATADIR}/tubify/fonts)
    endif()
    # the font's list of names, from the same commit: the tests check every
    # symbol the qml asks for is in it, since a misspelt one shows as text
    set(SWIFTIFY_ICON_CODEPOINTS ${SWIFTIFY_FONT_DIRECTORY}/MaterialSymbolsRounded.codepoints)
    if(BUILD_TESTING)
        swiftify_download_asset(
            "https://raw.githubusercontent.com/google/material-design-icons/737e3324305806514d7909874fa1818ae1808232/variablefont/MaterialSymbolsRounded%5BFILL,GRAD,opsz,wght%5D.codepoints"
            "${SWIFTIFY_ICON_CODEPOINTS}"
            225bd09137103cb7746bc93dc08d08764c9f0c3bd04f4b958d4a3c3c19432dd6)
    endif()
endif()

# google sans flex, end4's face: the interface text, and the titles in it
# drawn round, wide and heavy through its variable axes. its licence travels
# with it.
option(SWIFTIFY_DOWNLOAD_INTERFACE_FONT
    "Download Google Sans Flex to install beside the app" ON)
if(SWIFTIFY_DOWNLOAD_INTERFACE_FONT)
    set(SWIFTIFY_INTERFACE_FONT ${SWIFTIFY_FONT_DIRECTORY}/GoogleSansFlex.ttf)
    set(SWIFTIFY_INTERFACE_FONT_LICENSE ${SWIFTIFY_FONT_DIRECTORY}/GoogleSansFlex-OFL.txt)
    set(SWIFTIFY_GOOGLE_FONTS https://raw.githubusercontent.com/google/fonts/a0e3dbcdc3a3ecfafff3f071159ae0221628922d/ofl/googlesansflex)
    swiftify_download_asset(
        "${SWIFTIFY_GOOGLE_FONTS}/GoogleSansFlex%5BGRAD,ROND,opsz,slnt,wdth,wght%5D.ttf"
        "${SWIFTIFY_INTERFACE_FONT}"
        c31a482fbecbf2e07e6890134d20078723aadf732c9b9c6c9a44f86f8265b6fe)
    swiftify_download_asset("${SWIFTIFY_GOOGLE_FONTS}/OFL.txt"
        "${SWIFTIFY_INTERFACE_FONT_LICENSE}"
        fc13d69f63e36d284b6e383d4d1463d8ad404f0aa065e57cf961252d51063137)
    if(NOT APPLE)
        install(FILES ${SWIFTIFY_INTERFACE_FONT} ${SWIFTIFY_INTERFACE_FONT_LICENSE}
            DESTINATION ${CMAKE_INSTALL_DATADIR}/tubify/fonts)
    endif()
endif()

# --- licences ------------------------------------------------------------------

# the code built into swiftify or bundled with it, whose licences go out with
# every copy beside its own. they're copied under their installed names first,
# as the app bundle can't rename what it carries
set(SWIFTIFY_LICENSE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/licenses)
configure_file(third_party/material-color-utilities/LICENSE
    ${SWIFTIFY_LICENSE_DIRECTORY}/material-color-utilities.LICENSE COPYONLY)
configure_file(third_party/rounded-polygon-qmljs/LICENSE
    ${SWIFTIFY_LICENSE_DIRECTORY}/rounded-polygon-qmljs.LICENSE COPYONLY)
set(SWIFTIFY_THIRD_PARTY_LICENSES
    ${SWIFTIFY_LICENSE_DIRECTORY}/material-color-utilities.LICENSE
    ${SWIFTIFY_LICENSE_DIRECTORY}/rounded-polygon-qmljs.LICENSE
    # the rust crates', made with cargo about (ytmusic-core/about.toml)
    ${CMAKE_CURRENT_SOURCE_DIR}/packaging/licenses/rust-crates.LICENSE)
# material symbols, the icon font downloaded above
if(SWIFTIFY_DOWNLOAD_ICON_FONT)
    list(APPEND SWIFTIFY_THIRD_PARTY_LICENSES ${CMAKE_CURRENT_SOURCE_DIR}/packaging/licenses/material-symbols.LICENSE)
endif()
# the prebuilt downloads bundle qt, qtkeychain and qcoro, and their licences go
# with them. a distro's package uses the system's and leaves them off
option(SWIFTIFY_BUNDLED_QT "Install the licences of Qt, QtKeychain and QCoro, for a build that bundles them" OFF)
if(SWIFTIFY_BUNDLED_QT)
    configure_file(packaging/licenses/qt.NOTICE.in ${SWIFTIFY_LICENSE_DIRECTORY}/qt.NOTICE @ONLY)
    list(APPEND SWIFTIFY_THIRD_PARTY_LICENSES
        ${CMAKE_CURRENT_SOURCE_DIR}/packaging/licenses/qt.LICENSE
        ${SWIFTIFY_LICENSE_DIRECTORY}/qt.NOTICE
        ${CMAKE_CURRENT_SOURCE_DIR}/packaging/licenses/qtkeychain.LICENSE
        ${CMAKE_CURRENT_SOURCE_DIR}/packaging/licenses/qcoro.LICENSE)
endif()

# --- the app ---------------------------------------------------------------------

file(GLOB_RECURSE SWIFTIFY_SOURCES CONFIGURE_DEPENDS RELATIVE ${CMAKE_CURRENT_SOURCE_DIR}
    src/*.cpp src/*.h src/*.mm)
list(REMOVE_ITEM SWIFTIFY_SOURCES src/main.cpp)
if(NOT APPLE)
    list(FILTER SWIFTIFY_SOURCES EXCLUDE REGEX "^src/system/macos/")
endif()
if(NOT WIN32)
    list(FILTER SWIFTIFY_SOURCES EXCLUDE REGEX "^src/system/windows/")
endif()
if(NOT SWIFTIFY_DESKTOP_INTEGRATION)
    list(FILTER SWIFTIFY_SOURCES EXCLUDE REGEX "^src/system/linux/")
endif()
if(NOT SWIFTIFY_DBUS)
    list(FILTER SWIFTIFY_SOURCES EXCLUDE REGEX "^src/system/dbus/")
endif()
if(NOT SWIFTIFY_SOLOIST)
    list(FILTER SWIFTIFY_SOURCES EXCLUDE REGEX "^src/soloist/")
else()
    if(NOT SWIFTIFY_DESKTOP_INTEGRATION)
        # routing soloist through swiftify records it with libpulse
        list(FILTER SWIFTIFY_SOURCES EXCLUDE REGEX "^src/soloist/SoloistAudioRoute")
    endif()
    if(NOT WIN32)
        # the visualizers hear soloist through a windows process loopback
        list(FILTER SWIFTIFY_SOURCES EXCLUDE REGEX "^src/soloist/SoloistLoopback")
    endif()
endif()

file(GLOB_RECURSE SWIFTIFY_QML CONFIGURE_DEPENDS RELATIVE ${CMAKE_CURRENT_SOURCE_DIR}
    qml/*.qml)
file(GLOB SWIFTIFY_SINGLETONS CONFIGURE_DEPENDS RELATIVE ${CMAKE_CURRENT_SOURCE_DIR}
    qml/design/*.qml)
set_source_files_properties(${SWIFTIFY_SINGLETONS} PROPERTIES QT_QML_SINGLETON_TYPE TRUE)

file(GLOB SWIFTIFY_SHADERS CONFIGURE_DEPENDS RELATIVE ${CMAKE_CURRENT_SOURCE_DIR}
    shaders/*.frag shaders/*.vert)

qt_add_library(swiftify_app STATIC)
qt_add_resources(swiftify_app "swiftify_translations"
    PREFIX "/translations"
    BASE translations
    FILES translations/ru.json)
qt_add_qml_module(swiftify_app
    URI Tubify
    VERSION 1.0
    SOURCES ${SWIFTIFY_SOURCES}
    QML_FILES ${SWIFTIFY_QML}
    RESOURCES resources/tubify-icon.png resources/cameo.png
    DEPENDENCIES QtQuick)
# BATCHABLE: the spectrum's material can be merged into the scene graph's
# batches, which needs the extra vertex input qsb adds for it.
qt_add_shaders(swiftify_app "swiftify_shaders"
    BATCHABLE
    PREFIX "/shaders"
    BASE shaders
    FILES ${SWIFTIFY_SHADERS})

# material 3's shapes as distance fields, for MaterialShape: drawn at build
# time from the vendored rounded-polygon-qmljs (tools/shape_atlas)
# the tool runs during the build, so a cross build (windows from a mac, say)
# takes one built for the machine doing the building
set(SWIFTIFY_SHAPE_ATLAS_TOOL "" CACHE FILEPATH
    "a shape_atlas built for the build machine, for cross builds")
if(SWIFTIFY_SHAPE_ATLAS_TOOL)
    set(SWIFTIFY_SHAPE_ATLAS_COMMAND ${SWIFTIFY_SHAPE_ATLAS_TOOL})
elseif(CMAKE_CROSSCOMPILING)
    message(FATAL_ERROR "a cross build needs -DSWIFTIFY_SHAPE_ATLAS_TOOL=<shape_atlas built for this machine>")
else()
    add_executable(shape_atlas tools/shape_atlas/main.cpp tools/shape_atlas/ShapeAtlasLayout.h)
    target_include_directories(shape_atlas PRIVATE tools/shape_atlas)
    target_link_libraries(shape_atlas PRIVATE Qt6::Core Qt6::Gui Qt6::Qml)
    # it runs on every fresh build, so it is optimised even in a debug one
    target_compile_options(shape_atlas PRIVATE $<$<CXX_COMPILER_ID:Clang,AppleClang,GNU>:-O2>)
    set(SWIFTIFY_SHAPE_ATLAS_COMMAND shape_atlas)
endif()
set(SWIFTIFY_SHAPE_LIBRARY ${CMAKE_CURRENT_SOURCE_DIR}/third_party/rounded-polygon-qmljs)
file(GLOB_RECURSE SWIFTIFY_SHAPE_SOURCES CONFIGURE_DEPENDS ${SWIFTIFY_SHAPE_LIBRARY}/*.js)
set(SWIFTIFY_SHAPE_ATLAS ${CMAKE_CURRENT_BINARY_DIR}/shapes/shape_atlas.png)
add_custom_command(OUTPUT ${SWIFTIFY_SHAPE_ATLAS}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_CURRENT_BINARY_DIR}/shapes
    COMMAND ${SWIFTIFY_SHAPE_ATLAS_COMMAND} ${SWIFTIFY_SHAPE_LIBRARY} ${SWIFTIFY_SHAPE_ATLAS}
    DEPENDS ${SWIFTIFY_SHAPE_ATLAS_COMMAND} ${SWIFTIFY_SHAPE_SOURCES}
    COMMENT "Drawing the shape atlas")
qt_add_resources(swiftify_app "swiftify_shape_atlas"
    PREFIX "/shapes"
    BASE ${CMAKE_CURRENT_BINARY_DIR}/shapes
    FILES ${SWIFTIFY_SHAPE_ATLAS})

target_include_directories(swiftify_app PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/src)
# qmltyperegistrar includes registered headers by file name alone, so every
# folder holding one is searched too. header names stay unique across src/.
set(SWIFTIFY_SOURCE_DIRECTORIES)
foreach(source IN LISTS SWIFTIFY_SOURCES)
    get_filename_component(directory ${CMAKE_CURRENT_SOURCE_DIR}/${source} DIRECTORY)
    list(APPEND SWIFTIFY_SOURCE_DIRECTORIES ${directory})
endforeach()
list(REMOVE_DUPLICATES SWIFTIFY_SOURCE_DIRECTORIES)
target_include_directories(swiftify_app PRIVATE ${SWIFTIFY_SOURCE_DIRECTORIES})
# MaterialShapeItem finds a shape's cell the way the atlas tool lays them out
target_include_directories(swiftify_app PRIVATE tools/shape_atlas)
# public so main.cpp, built in the swiftify target, starts mpris and the
# sleep and device monitors
target_compile_definitions(swiftify_app PUBLIC
    SWIFTIFY_VERSION="${PROJECT_VERSION}"
    $<$<BOOL:${SWIFTIFY_DESKTOP_INTEGRATION}>:SWIFTIFY_DESKTOP_INTEGRATION>
    $<$<BOOL:${SWIFTIFY_DBUS}>:SWIFTIFY_HAS_DBUS>)
target_compile_definitions(swiftify_app PRIVATE
    SWIFTIFY_BUILD_FONT_DIRECTORY="${SWIFTIFY_FONT_DIRECTORY}")
# main.cpp picks the core, and is built outside swiftify_app
target_compile_definitions(swiftify_app PUBLIC $<$<BOOL:${SWIFTIFY_SOLOIST}>:SWIFTIFY_HAS_SOLOIST>)
if(SWIFTIFY_DBUS)
    target_link_libraries(swiftify_app PUBLIC Qt6::DBus)
endif()
if(SWIFTIFY_SOLOIST)
    target_link_libraries(swiftify_app PUBLIC Qt6::WebSockets)
endif()
if(SWIFTIFY_DESKTOP_INTEGRATION)
    target_link_libraries(swiftify_app PUBLIC PkgConfig::PULSE)
endif()
if(APPLE)
    target_link_libraries(swiftify_app PUBLIC "-framework AppKit" "-framework MediaPlayer" "-framework QuartzCore")
    # now playing holds its objective-c objects with arc
    set_source_files_properties(src/system/macos/NowPlaying.mm src/system/macos/MacWindow.mm
        PROPERTIES COMPILE_OPTIONS -fobjc-arc)
endif()
target_link_libraries(swiftify_app PUBLIC
    Qt6::Core Qt6::Gui Qt6::Network Qt6::Concurrent Qt6::Qml Qt6::Quick Qt6::QuickControls2
    QCoro6::Core QCoro6::Network
    material_color_utilities
    ytmusic_core)

if(WIN32)
    target_link_libraries(swiftify_app PUBLIC Qt6::Widgets comctl32 dwmapi ole32 shell32 uuid runtimeobject mmdevapi)

    # older mingw-w64 headers (qt's bundled mingw 13.1, say) have no
    # SystemMediaTransportControls in windows.media.h; the media controls are
    # left out there, and the taskbar and window styling still build
    include(CheckCXXSourceCompiles)
    set(CMAKE_REQUIRED_LIBRARIES runtimeobject)
    check_cxx_source_compiles([=[
        #ifdef __MINGW32__
        #define ____FIReference_1_boolean_INTERFACE_DEFINED__
        #endif
        #include <windows.h>
        #include <roapi.h>
        #include <windows.media.h>
        #include <systemmediatransportcontrolsinterop.h>
        using Handler = ABI::Windows::Foundation::ITypedEventHandler<
            ABI::Windows::Media::SystemMediaTransportControls *,
            ABI::Windows::Media::SystemMediaTransportControlsButtonPressedEventArgs *>;
        int main()
        {
            ABI::Windows::Media::ISystemMediaTransportControls *controls = nullptr;
            ABI::Windows::Media::IMusicDisplayProperties2 *music = nullptr;
            ISystemMediaTransportControlsInterop *interop = nullptr;
            Handler *handler = nullptr;
            return __uuidof(Handler).Data1 != 0 && !controls && !music && !interop && !handler ? 0 : 1;
        }
    ]=] SWIFTIFY_HAVE_SMTC)
    unset(CMAKE_REQUIRED_LIBRARIES)
    if(SWIFTIFY_HAVE_SMTC)
        target_compile_definitions(swiftify_app PRIVATE SWIFTIFY_HAVE_SMTC)
    else()
        message(WARNING "This toolchain's windows.media.h has no SystemMediaTransportControls, so Tubify is "
            "building without the Windows media controls (media overlay, lock screen, media keys). "
            "Build with MSVC, or a newer mingw-w64, for them; see the Windows section of README.md.")
    endif()
endif()

qt_add_executable(swiftify src/main.cpp)
target_link_libraries(swiftify PRIVATE swiftify_app swiftify_appplugin)
if(SWIFTIFY_IS_LINUX)
    target_link_libraries(swiftify PRIVATE Qt6::Widgets)
endif()
# a windowed app on windows, with no console window of its own, and the icon
# in its resources so explorer and a pinned taskbar button show it
set_target_properties(swiftify PROPERTIES WIN32_EXECUTABLE ON)
if(WIN32)
    set_source_files_properties(packaging/icons/tubify.rc
        PROPERTIES OBJECT_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/packaging/icons/tubify.ico)
    target_sources(swiftify PRIVATE packaging/icons/tubify.rc)
endif()
if(MINGW)
    # qt's WinMain shim reads msvcrt's __argc, which a mingw on ucrt (as
    # homebrew's is) lacks; mingw's own start-up calls main() just as well
    set_target_properties(swiftify PROPERTIES qt_no_entrypoint ON)
endif()

# on macos, an app bundle: Tubify.app, with its own name in the menu bar and
# the dock, its own icon, and an identifier apart from the swift app's
# (com.addenator.Swiftify). the executable inside is Tubify too, which is the
# name activity monitor and codesign report
if(APPLE)
    set(SWIFTIFY_ICON_SOURCE ${CMAKE_CURRENT_SOURCE_DIR}/packaging/icons)
    # the build number the bundle carries: a tag's version, unless a build
    # numbers its own. apple wants one that only ever goes up
    set(SWIFTIFY_BUILD_NUMBER "" CACHE STRING
        "CFBundleVersion of the app bundle; the project version when empty")
    if(NOT SWIFTIFY_BUILD_NUMBER)
        set(SWIFTIFY_BUILD_NUMBER ${PROJECT_VERSION})
    endif()
    # an icon composer catalog (tubify.icon), compiled as xcode compiles it:
    # an asset catalog that macos 26 draws with its glass and its dark and
    # tinted looks, and an .icns for older systems. without one, an .icns
    # drawn from the rendered pngs, which every mac reads. the pngs are the
    # app's own: the swift app's catalog and pngs are left out, so the dock
    # and the finder show Tubify's icon at every size
    execute_process(COMMAND xcrun --find actool
        OUTPUT_VARIABLE SWIFTIFY_ACTOOL OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
    if(SWIFTIFY_ACTOOL AND EXISTS ${SWIFTIFY_ICON_SOURCE}/tubify.icon)
        set(SWIFTIFY_ICON_OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/icon)
        file(GLOB SWIFTIFY_ICON_PARTS LIST_DIRECTORIES false CONFIGURE_DEPENDS
            ${SWIFTIFY_ICON_SOURCE}/tubify.icon/* ${SWIFTIFY_ICON_SOURCE}/tubify.icon/Assets/*)
        add_custom_command(OUTPUT ${SWIFTIFY_ICON_OUTPUT}/Assets.car ${SWIFTIFY_ICON_OUTPUT}/tubify.icns
            COMMAND ${CMAKE_COMMAND} -E make_directory ${SWIFTIFY_ICON_OUTPUT}
            COMMAND ${SWIFTIFY_ACTOOL} --compile ${SWIFTIFY_ICON_OUTPUT} --platform macosx
                --minimum-deployment-target ${CMAKE_OSX_DEPLOYMENT_TARGET} --app-icon tubify
                --output-partial-info-plist ${SWIFTIFY_ICON_OUTPUT}/icon.plist
                ${SWIFTIFY_ICON_SOURCE}/tubify.icon
            DEPENDS ${SWIFTIFY_ICON_PARTS}
            COMMENT "Compiling the app icon")
        set(SWIFTIFY_ICON_FILES ${SWIFTIFY_ICON_OUTPUT}/Assets.car ${SWIFTIFY_ICON_OUTPUT}/tubify.icns)
        set(SWIFTIFY_ICON_FILE_NAME tubify.icns)
        # the icon the system finds in the asset catalog. a bundle that names
        # one and carries no catalog shows no icon at all, so it is only named
        # where one was compiled
        set(SWIFTIFY_ICON_NAME_KEY "<key>CFBundleIconName</key>\n    <string>tubify</string>")
    else()
        set(SWIFTIFY_ICONSET ${CMAKE_CURRENT_BINARY_DIR}/Tubify.iconset)
        set(SWIFTIFY_ICNS ${CMAKE_CURRENT_BINARY_DIR}/Tubify.icns)
        # iconutil takes a folder of exactly these names and sizes; the 1024
        # pixel one a retina 512 wants is drawn from the 512
        add_custom_command(OUTPUT ${SWIFTIFY_ICNS}
            COMMAND ${CMAKE_COMMAND} -E make_directory ${SWIFTIFY_ICONSET}
            COMMAND sips -z 16 16 ${SWIFTIFY_ICON_SOURCE}/tubify-32.png --out ${SWIFTIFY_ICONSET}/icon_16x16.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-32.png ${SWIFTIFY_ICONSET}/icon_16x16@2x.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-32.png ${SWIFTIFY_ICONSET}/icon_32x32.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-64.png ${SWIFTIFY_ICONSET}/icon_32x32@2x.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-128.png ${SWIFTIFY_ICONSET}/icon_128x128.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-256.png ${SWIFTIFY_ICONSET}/icon_128x128@2x.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-256.png ${SWIFTIFY_ICONSET}/icon_256x256.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-512.png ${SWIFTIFY_ICONSET}/icon_256x256@2x.png
            COMMAND ${CMAKE_COMMAND} -E copy ${SWIFTIFY_ICON_SOURCE}/tubify-512.png ${SWIFTIFY_ICONSET}/icon_512x512.png
            COMMAND sips -z 1024 1024 ${SWIFTIFY_ICON_SOURCE}/tubify-512.png --out ${SWIFTIFY_ICONSET}/icon_512x512@2x.png
            COMMAND iconutil --convert icns --output ${SWIFTIFY_ICNS} ${SWIFTIFY_ICONSET}
            DEPENDS ${SWIFTIFY_ICON_SOURCE}/tubify-32.png ${SWIFTIFY_ICON_SOURCE}/tubify-64.png
                ${SWIFTIFY_ICON_SOURCE}/tubify-128.png ${SWIFTIFY_ICON_SOURCE}/tubify-256.png
                ${SWIFTIFY_ICON_SOURCE}/tubify-512.png
            COMMENT "Making the app icon")
        set(SWIFTIFY_ICON_FILES ${SWIFTIFY_ICNS})
        set(SWIFTIFY_ICON_FILE_NAME Tubify.icns)
        set(SWIFTIFY_ICON_NAME_KEY "")
    endif()
    set_source_files_properties(${SWIFTIFY_ICON_FILES} PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
    target_sources(swiftify PRIVATE ${SWIFTIFY_ICON_FILES})
    if(SWIFTIFY_ICON_FONT)
        set_source_files_properties(${SWIFTIFY_ICON_FONT} PROPERTIES MACOSX_PACKAGE_LOCATION Resources/fonts)
        target_sources(swiftify PRIVATE ${SWIFTIFY_ICON_FONT})
    endif()
    if(SWIFTIFY_INTERFACE_FONT)
        set_source_files_properties(${SWIFTIFY_INTERFACE_FONT} ${SWIFTIFY_INTERFACE_FONT_LICENSE}
            PROPERTIES MACOSX_PACKAGE_LOCATION Resources/fonts)
        target_sources(swiftify PRIVATE ${SWIFTIFY_INTERFACE_FONT} ${SWIFTIFY_INTERFACE_FONT_LICENSE})
    endif()
    set_source_files_properties(LICENSE PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
    target_sources(swiftify PRIVATE LICENSE)
    set_source_files_properties(${SWIFTIFY_THIRD_PARTY_LICENSES} PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
    target_sources(swiftify PRIVATE ${SWIFTIFY_THIRD_PARTY_LICENSES})
    # the bundle's Info.plist, from the template: the MACOSX_BUNDLE_* the
    # build sets are filled in by cmake itself, and what the template names
    # with @...@ here
    file(MAKE_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/macos)
    set(SWIFTIFY_MACOS_INFO_PLIST ${CMAKE_CURRENT_BINARY_DIR}/macos/Info.plist)
    # the executable inside the bundle, which is not the target's name
    set(SWIFTIFY_BUNDLE_EXECUTABLE Tubify)
    set(SWIFTIFY_BUNDLE_MINIMUM_MACOS ${CMAKE_OSX_DEPLOYMENT_TARGET})
    configure_file(packaging/macos/Info.plist.in ${SWIFTIFY_MACOS_INFO_PLIST} @ONLY)
    set_target_properties(swiftify PROPERTIES
        MACOSX_BUNDLE ON
        OUTPUT_NAME ${SWIFTIFY_BUNDLE_EXECUTABLE}
        MACOSX_BUNDLE_INFO_PLIST ${SWIFTIFY_MACOS_INFO_PLIST}
        MACOSX_BUNDLE_BUNDLE_NAME Tubify
        MACOSX_BUNDLE_GUI_IDENTIFIER dev.hinikaa.Tubify
        MACOSX_BUNDLE_ICON_FILE ${SWIFTIFY_ICON_FILE_NAME}
        MACOSX_BUNDLE_SHORT_VERSION_STRING ${PROJECT_VERSION}
        MACOSX_BUNDLE_BUNDLE_VERSION ${SWIFTIFY_BUILD_NUMBER}
        MACOSX_BUNDLE_COPYRIGHT "Copyright 2026 Hinika. Licensed under the GNU GPL v3 or later")

    # signing. the linker signs only the executable; signing the bundle seals
    # the icon and the font in with it, so the whole app checks out. a folder
    # synced by icloud tags files with finder and file-provider attributes,
    # which codesign refuses to seal, so they go first.
    # ad hoc by default, which is enough to run it here. an identity of one's
    # own keeps the signature's requirement the same from build to build, so
    # the keychain hands over the stored logins without asking again after
    # each one, and is what notarising needs
    set(SWIFTIFY_CODESIGN_IDENTITY "-" CACHE STRING "codesign identity for the app bundle; - signs ad hoc")
    option(SWIFTIFY_CODESIGN "Sign the app bundle after linking" ON)
    option(SWIFTIFY_HARDENED_RUNTIME
        "Sign with the hardened runtime and packaging/macos/Tubify.entitlements, as notarising needs" OFF)
    set(SWIFTIFY_CODESIGN_FLAGS --force --sign ${SWIFTIFY_CODESIGN_IDENTITY})
    if(SWIFTIFY_HARDENED_RUNTIME)
        list(APPEND SWIFTIFY_CODESIGN_FLAGS --options runtime
            --entitlements ${CMAKE_CURRENT_SOURCE_DIR}/packaging/macos/Tubify.entitlements)
    endif()
    if(SWIFTIFY_CODESIGN)
        add_custom_command(TARGET swiftify POST_BUILD
            COMMAND xattr -cr $<TARGET_BUNDLE_DIR:swiftify>
            COMMAND codesign ${SWIFTIFY_CODESIGN_FLAGS} $<TARGET_BUNDLE_DIR:swiftify>
            COMMENT "Signing the app bundle")
    endif()

    # qt's frameworks, plugins and qml modules, so the bundle runs on a mac
    # without qt installed. macdeployqt is looked for beside the qmake the
    # build found, never on the path: a homebrew qt would bundle its own qt
    # over the kit's. off leaves the bundle linked against the qt it was built
    # with, for running it from the build tree while working on it
    option(SWIFTIFY_MACDEPLOYQT "Bundle Qt into the app with macdeployqt, at install and package time" ON)
    if(TARGET Qt6::macdeployqt)
        get_target_property(SWIFTIFY_MACDEPLOYQT_BIN Qt6::macdeployqt IMPORTED_LOCATION)
    else()
        get_target_property(SWIFTIFY_QMAKE Qt6::qmake IMPORTED_LOCATION)
        get_filename_component(SWIFTIFY_QT_BIN "${SWIFTIFY_QMAKE}" DIRECTORY)
        if(EXISTS "${SWIFTIFY_QT_BIN}/macdeployqt")
            set(SWIFTIFY_MACDEPLOYQT_BIN "${SWIFTIFY_QT_BIN}/macdeployqt")
        endif()
    endif()
    if(SWIFTIFY_MACDEPLOYQT AND NOT SWIFTIFY_MACDEPLOYQT_BIN)
        message(WARNING "No macdeployqt in the Qt kit, so the app bundle carries no Qt and runs only on a "
            "mac with this Qt installed. Point CMAKE_PREFIX_PATH at a full Qt kit.")
    endif()
    if(SWIFTIFY_MACDEPLOYQT AND SWIFTIFY_MACDEPLOYQT_BIN)
        # for working on it: bundles qt into the build tree's bundle
        add_custom_target(macdeploy
            COMMAND ${SWIFTIFY_MACDEPLOYQT_BIN} $<TARGET_BUNDLE_DIR:swiftify>
                -qmldir=${CMAKE_CURRENT_SOURCE_DIR}/qml -always-overwrite
            COMMENT "Bundling Qt into the app")
        add_dependencies(macdeploy swiftify)
        # and for installing or packaging it: the installed bundle, in place,
        # before cpack wraps it. the script is run by the install rule below
        # the bundle's own, as it has nothing to deploy before that
        configure_file(packaging/macos/deploy-qt.cmake.in
            ${CMAKE_CURRENT_BINARY_DIR}/macos/deploy-qt.cmake @ONLY)
    endif()

    # the dmg opens to the app beside a link to applications, to drag it into;
    # the zip is the same bundle for a script to unpack
    if(CMAKE_OSX_ARCHITECTURES)
        string(REPLACE ";" "-" SWIFTIFY_PACKAGE_ARCH "${CMAKE_OSX_ARCHITECTURES}")
    else()
        execute_process(COMMAND uname -m OUTPUT_VARIABLE SWIFTIFY_PACKAGE_ARCH OUTPUT_STRIP_TRAILING_WHITESPACE)
    endif()
    set(CPACK_GENERATOR "DragNDrop;ZIP")
    set(CPACK_PACKAGE_NAME "Tubify")
    set(CPACK_PACKAGE_VENDOR "Hinika")
    set(CPACK_PACKAGE_CONTACT "Hinika")
    set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "${PROJECT_DESCRIPTION}")
    set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
    set(CPACK_PACKAGE_FILE_NAME "Tubify-${PROJECT_VERSION}-macos-${SWIFTIFY_PACKAGE_ARCH}")
    # the bundle sits at the root of the package, where a dmg shows it beside
    # the applications link to drag it into
    set(CPACK_PACKAGING_INSTALL_PREFIX "/")
    set(CPACK_DMG_VOLUME_NAME "Tubify")
    set(CPACK_DMG_FORMAT "UDZO")
    set(CPACK_DMG_DISABLE_APPLICATIONS_SYMLINK OFF)
    set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/LICENSE")
    # notarising is apple's, and needs a paid developer identity: off unless
    # a build asks for it and names the keychain profile notarytool stored
    option(SWIFTIFY_NOTARIZE "Notarise the packages with notarytool after cpack builds them" OFF)
    set(SWIFTIFY_NOTARY_PROFILE "" CACHE STRING "the notarytool keychain profile to notarise with")
    if(SWIFTIFY_NOTARIZE)
        if(NOT SWIFTIFY_NOTARY_PROFILE)
            message(FATAL_ERROR "SWIFTIFY_NOTARIZE needs -DSWIFTIFY_NOTARY_PROFILE=<profile>, as stored by "
                "xcrun notarytool store-credentials")
        endif()
        configure_file(packaging/macos/notarize.cmake.in
            ${CMAKE_CURRENT_BINARY_DIR}/macos/notarize.cmake @ONLY)
        set(CPACK_POST_BUILD_SCRIPTS ${CMAKE_CURRENT_BINARY_DIR}/macos/notarize.cmake)
    endif()
endif()


install(TARGETS swiftify
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
    BUNDLE DESTINATION .)
# after the bundle itself, which is what it fills with qt
if(APPLE AND SWIFTIFY_MACDEPLOYQT AND SWIFTIFY_MACDEPLOYQT_BIN)
    install(SCRIPT ${CMAKE_CURRENT_BINARY_DIR}/macos/deploy-qt.cmake)
endif()
# the gpl has to go out with every copy of swiftify, so the tarball and the
# windows zip carry it here and the app bundle in its resources
if(NOT APPLE)
    install(FILES LICENSE DESTINATION ${CMAKE_INSTALL_DATADIR}/licenses/tubify)
    install(FILES ${SWIFTIFY_THIRD_PARTY_LICENSES} DESTINATION ${CMAKE_INSTALL_DATADIR}/licenses/tubify)
endif()
if(NOT APPLE AND NOT WIN32)
    # the desktop entry's name is the wayland app id (see main.cpp), which is
    # how hyprland and the tray match the window to its icon
    install(FILES packaging/tubify.desktop DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
    foreach(size 32 48 64 128 256 512)
        install(FILES packaging/icons/tubify-${size}.png
            DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/${size}x${size}/apps
            RENAME tubify.png)
    endforeach()
endif()

# after every install rule above, so cpack's staging run carries them all.
# only macos has a package: the linux tarball and the windows zip are the
# scripts in tools/, which bundle more than cpack's install rules do
if(APPLE)
    include(CPack)
endif()
