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

#pragma once

#include <QDateTime>
#include <QHash>
#include <QString>
#include <QCoroTask>

#include <optional>

namespace swiftify {

/// a saved login. the json the apple apps keep in their keychain, and more
/// that they ignore.
struct StoredCredentials {
    QString clientId;
    /// spotify rotates it on every refresh and revokes it if an old one is
    /// used again, so it is spent as rarely as possible
    QString refreshToken;
    /// 0 when an old build saved it without one
    int authorizationVersion = 0;
    /// playback only: librespot's own login for the session (its reusable
    /// credentials), which reconnects spending no refresh token at all
    QString playbackLogin;
    /// the access token last issued and when it runs out: a launch inside
    /// its hour uses it instead of spending the refresh token
    QString accessToken;
    QDateTime accessTokenExpiration;

    QByteArray toJson() const;
    static std::optional<StoredCredentials> fromJson(const QByteArray &json);
};

/// where the two refresh tokens live. throws SpotifyError when the store
/// itself fails; a login that was never saved is nullopt.
class CredentialStore {
public:
    enum class Account { Playback, Library };

    virtual ~CredentialStore() = default;
    virtual QCoro::Task<std::optional<StoredCredentials>> load(Account account) = 0;
    virtual QCoro::Task<> save(Account account, StoredCredentials credentials) = 0;
    virtual QCoro::Task<> removeAll() = 0;

    /// the keychain item names the apple apps use. a development build on a mac
    /// would otherwise share (and rotate) the swift app's own login.
    static QString service()
    {
#ifdef Q_OS_MACOS
        return QStringLiteral("dev.addenator.Swiftify.spotify.qt-development");
#else
        return QStringLiteral("dev.addenator.Swiftify.spotify");
#endif
    }
    static QString accountName(Account account);
};

/// the secret service (gnome keyring, kwallet, keepassxc) through qtkeychain.
class KeychainCredentialStore : public CredentialStore {
public:
    QCoro::Task<std::optional<StoredCredentials>> load(Account account) override;
    QCoro::Task<> save(Account account, StoredCredentials credentials) override;
    QCoro::Task<> removeAll() override;
};

/// kept in memory, for tests.
class MemoryCredentialStore : public CredentialStore {
public:
    QCoro::Task<std::optional<StoredCredentials>> load(Account account) override;
    QCoro::Task<> save(Account account, StoredCredentials credentials) override;
    QCoro::Task<> removeAll() override;

    QHash<Account, StoredCredentials> items;
};

} // namespace swiftify
