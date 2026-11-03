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

#include "spotify/CredentialStore.h"

#include "spotify/SpotifyError.h"

#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>
#include <QFileInfo>

namespace swiftify {

namespace {

SpotifyError storeError(const QString &operation, const QString &detail)
{
    return {SpotifyError::Kind::Credentials,
            QStringLiteral("Could not %1 the saved YouTube Music login: %2").arg(operation, detail)};
}

QString storePath()
{
    const QString directory = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    return QDir(directory).filePath(QStringLiteral("login.json"));
}

} // namespace

QByteArray StoredCredentials::toJson() const
{
    return QJsonDocument(QJsonObject {{QStringLiteral("login"), login}}).toJson(QJsonDocument::Compact);
}

std::optional<StoredCredentials> StoredCredentials::fromJson(const QByteArray &json)
{
    const QJsonObject object = QJsonDocument::fromJson(json).object();
    const QString login = object.value(QStringLiteral("login")).toString();
    if (login.isEmpty()) {
        return std::nullopt;
    }
    return StoredCredentials {login};
}

QString CredentialStore::service()
{
    return QStringLiteral("dev.hinikaa.Tubify.youtube-music");
}

QString CredentialStore::accountName(Account account)
{
    return account == Account::Playback ? QStringLiteral("youtube-music-login")
                                        : QStringLiteral("youtube-music-library");
}

QCoro::Task<std::optional<StoredCredentials>> FileCredentialStore::load(Account account)
{
    if (account != Account::Playback) {
        co_return std::nullopt;
    }
    QFile file(storePath());
    if (!file.exists()) {
        co_return std::nullopt;
    }
    if (!file.open(QIODevice::ReadOnly)) {
        throw storeError(QStringLiteral("read"), file.errorString());
    }
    const auto credentials = StoredCredentials::fromJson(file.readAll());
    if (!credentials) {
        throw storeError(QStringLiteral("read"), QStringLiteral("it is damaged."));
    }
    co_return credentials;
}

QCoro::Task<> FileCredentialStore::save(Account account, StoredCredentials credentials)
{
    if (account != Account::Playback) {
        co_return;
    }
    const QString path = storePath();
    if (!QDir().mkpath(QFileInfo(path).absolutePath())) {
        throw storeError(QStringLiteral("write"), QStringLiteral("its folder could not be made."));
    }
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        throw storeError(QStringLiteral("write"), file.errorString());
    }
    if (file.write(credentials.toJson()) < 0) {
        throw storeError(QStringLiteral("write"), file.errorString());
    }
    file.close();
    QFile::setPermissions(path, QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    co_return;
}

QCoro::Task<> FileCredentialStore::removeAll()
{
    const QString path = storePath();
    if (QFile::exists(path) && !QFile::remove(path)) {
        throw storeError(QStringLiteral("remove"), QStringLiteral("the file is still there."));
    }
    co_return;
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
