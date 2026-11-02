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

#pragma once

#include "core/Async.h"
#include "spotify/CredentialStore.h"
#include "spotify/SpotifyError.h"
#include "spotify/WebApi.h"

#include <QObject>
#include <QUrl>
#include <QtQml/qqmlregistration.h>

#include <functional>
#include <optional>
#include <type_traits>
#include <utility>

class QQmlEngine;
class QJSEngine;

namespace swiftify {

class ErrorCenter;
class PlaybackCore;

/// the YouTube Music login: one browser cookie, kept as the core's own login
/// and reused at every launch. the class keeps the name the views already
/// refer to (`Session`), but there is only one account now: the cookie opens
/// both the library and playback.
class SpotifySession : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(Session)
    QML_SINGLETON

    Q_PROPERTY(QString status READ status NOTIFY statusChanged FINAL)
    Q_PROPERTY(QString libraryStatus READ libraryStatus NOTIFY libraryStatusChanged FINAL)
    Q_PROPERTY(bool connected READ isConnected NOTIFY connectedChanged FINAL)
    Q_PROPERTY(bool libraryConnected READ isLibraryConnected NOTIFY libraryConnectedChanged FINAL)
    Q_PROPERTY(bool libraryNeedsAuthorizationUpgrade READ libraryNeedsAuthorizationUpgrade NOTIFY libraryConnectedChanged FINAL)
    Q_PROPERTY(bool hasFullLibraryAccess READ hasFullLibraryAccess NOTIFY libraryConnectedChanged FINAL)
    Q_PROPERTY(bool hasLibraryToken READ hasLibraryToken NOTIFY libraryConnectedChanged FINAL)
    Q_PROPERTY(bool evaluatingStoredLogin READ isEvaluatingStoredLogin NOTIFY evaluatingStoredLoginChanged FINAL)
    Q_PROPERTY(QString libraryClientId READ libraryClientId WRITE setLibraryClientId NOTIFY libraryClientIdChanged FINAL)
    Q_PROPERTY(bool loginInProgress READ isLoginInProgress NOTIFY loginChanged FINAL)
    Q_PROPERTY(bool loginIsForLibrary READ isLoginForLibrary NOTIFY loginChanged FINAL)
    Q_PROPERTY(QUrl authorizationUrl READ authorizationUrl NOTIFY loginChanged FINAL)
    Q_PROPERTY(QString redirectUri READ redirectUri CONSTANT FINAL)

public:
    SpotifySession(PlaybackCore *core, CredentialStore *credentials, ErrorCenter *errors,
                   QObject *parent = nullptr);
    ~SpotifySession() override;

    static SpotifySession *instance();
    static SpotifySession *create(QQmlEngine *, QJSEngine *);

    QString status() const { return m_status; }
    QString libraryStatus() const { return m_libraryStatus; }
    bool isConnected() const { return m_connected; }
    bool isLibraryConnected() const { return m_libraryConnected; }
    bool libraryNeedsAuthorizationUpgrade() const { return false; }
    bool hasFullLibraryAccess() const { return m_libraryConnected; }
    bool hasLibraryToken() const { return m_connected; }
    bool hasLibraryRefreshToken() const { return m_connected; }
    bool isEvaluatingStoredLogin() const { return m_evaluatingStoredLogin; }
    QString libraryClientId() const { return QString(); }
    void setLibraryClientId(const QString &) {}
    bool isLoginInProgress() const { return m_loginInProgress; }
    bool isLoginForLibrary() const { return false; }
    QUrl authorizationUrl() const { return QUrl(QStringLiteral("https://music.youtube.com")); }
    QString redirectUri() const { return QString(); }

    /// signs in with the Cookie header copied from a signed-in
    /// music.youtube.com. the whole flow is one request to check it.
    Q_INVOKABLE void signIn(const QString &cookie);
    /// the views' old buttons: they now just ask for the cookie.
    Q_INVOKABLE void beginPlaybackLogin();
    Q_INVOKABLE void beginLibraryLogin();
    Q_INVOKABLE void reopenLoginPage();
    Q_INVOKABLE void cancelLogin();
    Q_INVOKABLE void logOut();

    /// reads the saved login and reconnects it. called once at launch.
    void restoreSavedSessions();
    void flushCredentialSaves() {}
    void setBackgroundOpener(std::function<void(const QUrl &)>) {}

    void setStatus(const QString &status);
    void setLibraryStatus(const QString &status);

    WebApi &webApi() { return m_webApi; }
    PlaybackCore *core() const { return m_core; }
    /// bumped by a logout. work started before one must not touch what came after.
    quint64 generation() const { return m_generation; }
    void throwIfStale(quint64 generation) const
    {
        if (generation != m_generation) {
            throw Cancelled();
        }
    }

    template <typename T>
    QCoro::Task<T> withLibraryAccessToken(std::function<QCoro::Task<T>(QString)> operation);

    /// reconnects the core when its session is stale, invalid or suspect, or
    /// when `force` asks.
    QCoro::Task<> ensureFreshPlaybackConnection(bool force = false);
    void markPlaybackConnectionSuspect() { m_playbackConnectionIsSuspect = true; }

    /// what a login or restore runs before it counts as finished: profile,
    /// playlists and home, owned by the library.
    void setInitialLibraryLoader(std::function<QCoro::Task<>()> loader) { m_loadInitialLibrary = std::move(loader); }

signals:
    void statusChanged();
    void libraryStatusChanged();
    void connectedChanged();
    void libraryConnectedChanged();
    void evaluatingStoredLoginChanged();
    void libraryClientIdChanged();
    void loginChanged();
    /// a view asked for the cookie dialog
    void loginRequested();
    /// the core has a fresh session
    void playbackConnected();
    /// the core was replaced and every login forgotten. reset everything.
    void loggedOut();
    void credentialsSaved();

private:
    QCoro::Task<> restore();
    QCoro::Task<> signInAsync(QString cookie);
    QCoro::Task<> reconnect();
    QCoro::Task<> loadInitialLibrary();
    void adoptConnected();
    void adoptDisconnected();
    void reportError(const QString &message);

    PlaybackCore *m_core;
    CredentialStore *m_credentials;
    ErrorCenter *m_errors;
    WebApi m_webApi;
    std::function<QCoro::Task<>()> m_loadInitialLibrary;

    quint64 m_generation = 0;
    QString m_status = QStringLiteral("Disconnected");
    QString m_libraryStatus = QStringLiteral("Not connected");
    bool m_connected = false;
    bool m_libraryConnected = false;
    bool m_evaluatingStoredLogin = true;
    bool m_loginInProgress = false;
    bool m_playbackConnectionIsSuspect = false;
    QString m_reportedError;
};

template <typename T>
QCoro::Task<T> SpotifySession::withLibraryAccessToken(std::function<QCoro::Task<T>(QString)> operation)
{
    const quint64 generation = m_generation;
    if (!m_connected) {
        throw SpotifyError(SpotifyError::Kind::Session,
                           QStringLiteral("Sign in to YouTube Music to continue."));
    }
    if constexpr (std::is_void_v<T>) {
        co_await operation(QString());
        throwIfStale(generation);
        co_return;
    } else {
        T value = co_await operation(QString());
        throwIfStale(generation);
        co_return value;
    }
}

} // namespace swiftify
