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

#pragma once

#include <QHash>
#include <QString>
#include <QCoroTask>

#include <optional>

namespace swiftify {

/// the login the core hands back: the cookie and the account it belongs to,
/// as json. keeping it means the next launch needs no cookie again.
struct StoredCredentials {
    QString login;

    QByteArray toJson() const;
    static std::optional<StoredCredentials> fromJson(const QByteArray &json);
};

/// where the login lives. a login that was never saved is nullopt.
class CredentialStore {
public:
    enum class Account { Playback, Library };

    virtual ~CredentialStore() = default;
    virtual QCoro::Task<std::optional<StoredCredentials>> load(Account account) = 0;
    virtual QCoro::Task<> save(Account account, StoredCredentials credentials) = 0;
    virtual QCoro::Task<> removeAll() = 0;

    static QString service();
    static QString accountName(Account account);
};

/// a json file in the app's data directory, readable only by its owner.
class FileCredentialStore : public CredentialStore {
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
