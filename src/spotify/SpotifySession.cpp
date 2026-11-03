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

#include "spotify/SpotifySession.h"

#include "app/ErrorCenter.h"
#include "core/PlaybackCore.h"

#include <QCoreApplication>
#include <QLoggingCategory>
#include <QQmlEngine>
#include <QCoroSignal>

#include <utility>

namespace swiftify {

Q_LOGGING_CATEGORY(lcSession, "tubify.session")

namespace {
SpotifySession *s_instance = nullptr;
}

SpotifySession::SpotifySession(PlaybackCore *core, CredentialStore *credentials, ErrorCenter *errors,
                               QObject *parent)
    : QObject(parent)
    , m_core(core)
    , m_credentials(credentials)
    , m_errors(errors)
    , m_webApi(core)
{
    if (!s_instance) {
        s_instance = this;
    }
}

SpotifySession::~SpotifySession()
{
    if (s_instance == this) {
        s_instance = nullptr;
    }
}

SpotifySession *SpotifySession::instance()
{
    return s_instance;
}

SpotifySession *SpotifySession::create(QQmlEngine *, QJSEngine *)
{
    QJSEngine::setObjectOwnership(s_instance, QJSEngine::CppOwnership);
    return s_instance;
}

void SpotifySession::reportError(const QString &message)
{
    m_reportedError = message;
    if (m_errors) {
        m_errors->show(message);
    }
}

void SpotifySession::setStatus(const QString &status)
{
    if (m_status != status) {
        m_status = status;
        emit statusChanged();
    }
}

void SpotifySession::setLibraryStatus(const QString &status)
{
    if (m_libraryStatus != status) {
        m_libraryStatus = status;
        emit libraryStatusChanged();
    }
}

void SpotifySession::adoptConnected()
{
    const bool wasConnected = m_connected;
    const bool wasLibraryConnected = m_libraryConnected;
    m_connected = true;
    m_libraryConnected = true;
    setStatus(QStringLiteral("Connected"));
    setLibraryStatus(QStringLiteral("Connected"));
    if (!wasConnected) {
        emit connectedChanged();
    }
    if (!wasLibraryConnected) {
        emit libraryConnectedChanged();
    }
    emit playbackConnected();
}

void SpotifySession::adoptDisconnected()
{
    const bool wasConnected = m_connected;
    const bool wasLibraryConnected = m_libraryConnected;
    m_connected = false;
    m_libraryConnected = false;
    setStatus(QStringLiteral("Disconnected"));
    setLibraryStatus(QStringLiteral("Not connected"));
    if (wasConnected) {
        emit connectedChanged();
    }
    if (wasLibraryConnected) {
        emit libraryConnectedChanged();
    }
}

// --- launch ------------------------------------------------------------------------

void SpotifySession::restoreSavedSessions()
{
    detach(restore());
}

QCoro::Task<> SpotifySession::restore()
{
    const quint64 generation = m_generation;
    m_evaluatingStoredLogin = true;
    emit evaluatingStoredLoginChanged();

    std::optional<StoredCredentials> stored;
    try {
        stored = co_await m_credentials->load(CredentialStore::Account::Playback);
    } catch (const SpotifyError &error) {
        reportError(error.message());
        qCWarning(lcSession) << "reading the saved login:" << error.message();
    }
    if (generation != m_generation) {
        m_evaluatingStoredLogin = false;
        emit evaluatingStoredLoginChanged();
        co_return;
    }

    if (stored) {
        try {
            co_await m_core->connectStoredSession(stored->login, QString());
            throwIfStale(generation);
            adoptConnected();
            detach(loadInitialLibrary());
        } catch (const Cancelled &) {
        } catch (const SpotifyError &error) {
            reportError(QStringLiteral("The saved YouTube Music login has expired. Sign in again."));
            qCWarning(lcSession) << "reconnecting the saved login:" << error.message();
            adoptDisconnected();
        }
    } else {
        adoptDisconnected();
    }

    m_evaluatingStoredLogin = false;
    emit evaluatingStoredLoginChanged();
}

QCoro::Task<> SpotifySession::loadInitialLibrary()
{
    if (!m_loadInitialLibrary) {
        co_return;
    }
    try {
        co_await m_loadInitialLibrary();
    } catch (const Cancelled &) {
    } catch (const SpotifyError &error) {
        reportError(error.message());
    }
}

// --- signing in --------------------------------------------------------------------

void SpotifySession::signIn(const QString &cookie)
{
    detach(signInAsync(cookie));
}

QCoro::Task<> SpotifySession::signInAsync(QString cookie)
{
    if (m_loginInProgress) {
        co_return;
    }
    m_loginInProgress = true;
    emit loginChanged();
    setStatus(QStringLiteral("Signing in…"));

    const quint64 generation = m_generation;
    try {
        const QString login = co_await m_core->connectSession(cookie, QString());
        throwIfStale(generation);
        co_await m_credentials->save(CredentialStore::Account::Playback, StoredCredentials {login});
        emit credentialsSaved();
        adoptConnected();
        detach(loadInitialLibrary());
    } catch (const Cancelled &) {
    } catch (const SpotifyError &error) {
        reportError(error.message());
        adoptDisconnected();
    }

    m_loginInProgress = false;
    emit loginChanged();
}

void SpotifySession::signInWithBrowser(const QString &browser)
{
    detach(signInWithBrowserAsync(browser));
}

QCoro::Task<> SpotifySession::signInWithBrowserAsync(QString browser)
{
    if (m_loginInProgress) {
        co_return;
    }
    m_loginInProgress = true;
    emit loginChanged();
    setStatus(QStringLiteral("Reading your browser login…"));

    const quint64 generation = m_generation;
    try {
        const QString login = co_await m_core->connectBrowser(browser);
        throwIfStale(generation);
        co_await m_credentials->save(CredentialStore::Account::Playback, StoredCredentials {login});
        emit credentialsSaved();
        adoptConnected();
        detach(loadInitialLibrary());
    } catch (const Cancelled &) {
    } catch (const SpotifyError &error) {
        reportError(error.message());
        adoptDisconnected();
    }

    m_loginInProgress = false;
    emit loginChanged();
}

void SpotifySession::beginPlaybackLogin()
{
    setStatus(QStringLiteral("Paste your YouTube Music cookie to sign in."));
    emit loginChanged();
    emit loginRequested();
}

void SpotifySession::beginLibraryLogin()
{
    beginPlaybackLogin();
}

void SpotifySession::reopenLoginPage()
{
    emit loginChanged();
}

void SpotifySession::cancelLogin()
{
    if (m_loginInProgress) {
        m_loginInProgress = false;
        emit loginChanged();
    }
}

QCoro::Task<> SpotifySession::reconnect()
{
    const auto stored = co_await m_credentials->load(CredentialStore::Account::Playback);
    if (!stored) {
        throw SpotifyError(SpotifyError::Kind::Session,
                           QStringLiteral("Sign in to YouTube Music to play."));
    }
    co_await m_core->connectStoredSession(stored->login, QString());
    adoptConnected();
}

QCoro::Task<> SpotifySession::ensureFreshPlaybackConnection(bool force)
{
    if (!force && !m_playbackConnectionIsSuspect && m_core->sessionIsValid()) {
        co_return;
    }
    co_await reconnect();
    m_playbackConnectionIsSuspect = false;
}

void SpotifySession::logOut()
{
    ++m_generation;
    m_core->disconnectSession();
    adoptDisconnected();
    detach(m_credentials->removeAll());
    emit loggedOut();
}

} // namespace swiftify
