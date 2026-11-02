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

#include "spotify/CredentialStore.h"

#include "spotify/SpotifyError.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QTimeZone>
#include <qt6keychain/keychain.h>
#include <QCoroSignal>

namespace swiftify {

namespace {

SpotifyError storeError(const QString &operation, const QString &detail)
{
    return {SpotifyError::Kind::Credentials,
            QStringLiteral("Could not %1 the saved Spotify login: %2").arg(operation, detail)};
}

} // namespace

QByteArray StoredCredentials::toJson() const
{
    QJsonObject object {
        {QStringLiteral("clientID"), clientId},
        {QStringLiteral("refreshToken"), refreshToken},
        {QStringLiteral("authorizationVersion"), authorizationVersion},
    };
    if (!playbackLogin.isEmpty()) {
        object.insert(QStringLiteral("playbackLogin"), playbackLogin);
    }
    if (!accessToken.isEmpty() && accessTokenExpiration.isValid()) {
        object.insert(QStringLiteral("accessToken"), accessToken);
        object.insert(QStringLiteral("accessTokenExpiration"), accessTokenExpiration.toMSecsSinceEpoch());
    }
    return QJsonDocument(object).toJson(QJsonDocument::Compact);
}

std::optional<StoredCredentials> StoredCredentials::fromJson(const QByteArray &json)
{
    const QJsonObject object = QJsonDocument::fromJson(json).object();
    StoredCredentials credentials;
    credentials.clientId = object.value(QStringLiteral("clientID")).toString();
    credentials.refreshToken = object.value(QStringLiteral("refreshToken")).toString();
    credentials.authorizationVersion = object.value(QStringLiteral("authorizationVersion")).toInt();
    credentials.playbackLogin = object.value(QStringLiteral("playbackLogin")).toString();
    credentials.accessToken = object.value(QStringLiteral("accessToken")).toString();
    if (const qint64 expiration = object.value(QStringLiteral("accessTokenExpiration")).toInteger(); expiration > 0) {
        credentials.accessTokenExpiration = QDateTime::fromMSecsSinceEpoch(expiration, QTimeZone::UTC);
    }
    // a login spotify revoked can still play through librespot's own
    if (credentials.clientId.isEmpty() || (credentials.refreshToken.isEmpty() && credentials.playbackLogin.isEmpty())) {
        return std::nullopt;
    }
    return credentials;
}

QString CredentialStore::accountName(Account account)
{
    return account == Account::Playback ? QStringLiteral("spotify-oauth") : QStringLiteral("spotify-library-oauth");
}

QCoro::Task<std::optional<StoredCredentials>> KeychainCredentialStore::load(Account account)
{
    QKeychain::ReadPasswordJob job(service());
    job.setAutoDelete(false);
    job.setKey(accountName(account));
    job.start();
    co_await qCoro(&job, &QKeychain::Job::finished);

    if (job.error() == QKeychain::EntryNotFound) {
        co_return std::nullopt;
    }
    if (job.error() != QKeychain::NoError) {
        throw storeError(QStringLiteral("load"), job.errorString());
    }
    const auto credentials = StoredCredentials::fromJson(job.binaryData());
    if (!credentials) {
        throw storeError(QStringLiteral("load"), QStringLiteral("it is damaged."));
    }
    co_return credentials;
}

QCoro::Task<> KeychainCredentialStore::save(Account account, StoredCredentials credentials)
{
    QKeychain::WritePasswordJob job(service());
    job.setAutoDelete(false);
    job.setKey(accountName(account));
    job.setBinaryData(credentials.toJson());
    job.start();
    co_await qCoro(&job, &QKeychain::Job::finished);
    if (job.error() != QKeychain::NoError) {
        throw storeError(QStringLiteral("save"), job.errorString());
    }
}

QCoro::Task<> KeychainCredentialStore::removeAll()
{
    for (const Account account : {Account::Playback, Account::Library}) {
        QKeychain::DeletePasswordJob job(service());
        job.setAutoDelete(false);
        job.setKey(accountName(account));
        job.start();
        co_await qCoro(&job, &QKeychain::Job::finished);
        if (job.error() != QKeychain::NoError && job.error() != QKeychain::EntryNotFound) {
            throw storeError(QStringLiteral("delete"), job.errorString());
        }
    }
}

QCoro::Task<std::optional<StoredCredentials>> MemoryCredentialStore::load(Account account)
{
    const auto found = items.constFind(account);
    co_return found == items.cend() ? std::nullopt : std::optional(*found);
}

QCoro::Task<> MemoryCredentialStore::save(Account account, StoredCredentials credentials)
{
    items.insert(account, credentials);
    co_return;
}

QCoro::Task<> MemoryCredentialStore::removeAll()
{
    items.clear();
    co_return;
}

} // namespace swiftify
