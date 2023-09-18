// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use sqlx::SqliteConnection;
use sqlx::{sqlite::SqliteConnectOptions, Acquire, Pool, Sqlite};
use thiserror::Error;

use crate::db::Encoding;
use crate::package::{self, Meta};

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(path: impl AsRef<Path>, read_only: bool) -> Result<Self, Error> {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .read_only(read_only)
            .foreign_keys(true);

        Self::connect(options).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self, Error> {
        let pool = sqlx::SqlitePool::connect_with(options).await?;

        sqlx::migrate!("src/db/meta/migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn wipe(&self) -> Result<(), Error> {
        // Other tables cascade delete so we only need to truncate `meta`
        sqlx::query("DELETE FROM meta;").execute(&self.pool).await?;
        Ok(())
    }

    // TODO: Replace with specialized query interfaces
    pub async fn all(&self) -> Result<Vec<(package::Id, Meta)>, Error> {
        let entry_query = sqlx::query_as::<_, encoding::Entry>(
            "
            SELECT package,
                   name,
                   version_identifier,
                   source_release,
                   build_release,
                   architecture,
                   summary,
                   description,
                   source_id,
                   homepage,
                   uri,
                   hash,
                   download_size
            FROM meta;
            ",
        );

        let licenses_query = sqlx::query_as::<_, encoding::License>(
            "
            SELECT package, license
            FROM meta_licenses;
            ",
        );

        let dependencies_query = sqlx::query_as::<_, encoding::Dependency>(
            "
            SELECT package, dependency
            FROM meta_dependencies;
            ",
        );

        let providers_query = sqlx::query_as::<_, encoding::Provider>(
            "
            SELECT package, provider
            FROM meta_providers;
            ",
        );

        let (entries, licenses, dependencies, providers) = futures::try_join!(
            entry_query.fetch_all(&self.pool),
            licenses_query.fetch_all(&self.pool),
            dependencies_query.fetch_all(&self.pool),
            providers_query.fetch_all(&self.pool),
        )?;

        Ok(entries
            .into_iter()
            .map(|entry| {
                (
                    entry.id.0.clone(),
                    Meta {
                        name: entry.name.0,
                        version_identifier: entry.version_identifier,
                        source_release: entry.source_release as u64,
                        build_release: entry.build_release as u64,
                        architecture: entry.architecture,
                        summary: entry.summary,
                        description: entry.description,
                        source_id: entry.source_id,
                        homepage: entry.homepage,
                        licenses: licenses
                            .iter()
                            .filter(|l| l.id.0 == entry.id.0)
                            .map(|l| l.license.clone())
                            .collect(),
                        dependencies: dependencies
                            .iter()
                            .filter(|l| l.id.0 == entry.id.0)
                            .map(|d| d.dependency.0.clone())
                            .collect(),
                        providers: providers
                            .iter()
                            .filter(|l| l.id.0 == entry.id.0)
                            .map(|p| p.provider.0.clone())
                            .collect(),
                        uri: entry.uri,
                        hash: entry.hash,
                        download_size: entry.download_size.map(|i| i as u64),
                    },
                )
            })
            .collect())
    }

    pub async fn get(&self, package: &package::Id) -> Result<Meta, Error> {
        let entry_query = sqlx::query_as::<_, encoding::Entry>(
            "
            SELECT package, 
                   name,
                   version_identifier,
                   source_release,
                   build_release,
                   architecture,
                   summary,
                   description,
                   source_id,
                   homepage,
                   uri,
                   hash,
                   download_size
            FROM meta
            WHERE package = ?;
            ",
        )
        .bind(package.clone().encode());

        let licenses_query = sqlx::query_as::<_, encoding::License>(
            "
            SELECT package, license
            FROM meta_licenses
            WHERE package = ?;
            ",
        )
        .bind(package.clone().encode());

        let dependencies_query = sqlx::query_as::<_, encoding::Dependency>(
            "
            SELECT package, dependency
            FROM meta_dependencies
            WHERE package = ?;
            ",
        )
        .bind(package.clone().encode());

        let providers_query = sqlx::query_as::<_, encoding::Provider>(
            "
            SELECT package, provider
            FROM meta_providers
            WHERE package = ?;
            ",
        )
        .bind(package.clone().encode());

        let (entry, licenses, dependencies, providers) = futures::try_join!(
            entry_query.fetch_one(&self.pool),
            licenses_query.fetch_all(&self.pool),
            dependencies_query.fetch_all(&self.pool),
            providers_query.fetch_all(&self.pool),
        )?;

        Ok(Meta {
            name: entry.name.0,
            version_identifier: entry.version_identifier,
            source_release: entry.source_release as u64,
            build_release: entry.build_release as u64,
            architecture: entry.architecture,
            summary: entry.summary,
            description: entry.description,
            source_id: entry.source_id,
            homepage: entry.homepage,
            licenses: licenses.into_iter().map(|l| l.license).collect(),
            dependencies: dependencies.into_iter().map(|d| d.dependency.0).collect(),
            providers: providers.into_iter().map(|p| p.provider.0).collect(),
            uri: entry.uri,
            hash: entry.hash,
            download_size: entry.download_size.map(|i| i as u64),
        })
    }

    pub async fn add(&self, id: package::Id, meta: Meta) -> Result<(), Error> {
        let Meta {
            name,
            version_identifier,
            source_release,
            build_release,
            architecture,
            summary,
            description,
            source_id,
            homepage,
            licenses,
            dependencies,
            providers,
            uri,
            hash,
            download_size,
        } = meta;

        let mut transaction = self.pool.begin().await?;

        // Remove package (other tables cascade)
        remove(id.clone(), transaction.acquire().await?).await?;

        // Create entry
        sqlx::query(
            "
            INSERT INTO meta (
                package,
                name,
                version_identifier,
                source_release,
                build_release,
                architecture,
                summary,
                description,
                source_id,
                homepage,
                uri,
                hash,
                download_size                
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
        )
        .bind(id.clone().encode())
        .bind(name.encode())
        .bind(version_identifier)
        .bind(source_release as i64)
        .bind(build_release as i64)
        .bind(architecture)
        .bind(summary)
        .bind(description)
        .bind(source_id)
        .bind(homepage)
        .bind(uri)
        .bind(hash)
        .bind(download_size.map(|i| i as i64))
        .execute(transaction.acquire().await?)
        .await?;

        // Licenses
        if !licenses.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_licenses (package, license)
                ",
            )
            .push_values(licenses, |mut b, license| {
                b.push_bind(id.clone().encode()).push_bind(license);
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        // Dependencies
        if !dependencies.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_dependencies (package, dependency)
                ",
            )
            .push_values(dependencies, |mut b, dependency| {
                b.push_bind(id.clone().encode())
                    .push_bind(dependency.encode());
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        // Providers
        if !providers.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_providers (package, provider)
                ",
            )
            .push_values(providers, |mut b, provider| {
                b.push_bind(id.clone().encode())
                    .push_bind(provider.encode());
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        transaction.commit().await?;

        Ok(())
    }
}

async fn remove(package: package::Id, connection: &mut SqliteConnection) -> Result<(), Error> {
    sqlx::query(
        "
        DELETE FROM meta
        WHERE package = ?;
        ",
    )
    .bind(package.encode())
    .execute(connection)
    .await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("row not found")]
    RowNotFound,
    #[error("database error: {0}")]
    Sqlx(sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => Error::RowNotFound,
            error => Error::Sqlx(error),
        }
    }
}

mod encoding {
    use sqlx::FromRow;

    use crate::db::Decoder;
    use crate::package;

    #[derive(FromRow)]
    pub struct Entry {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub name: Decoder<package::Name>,
        pub version_identifier: String,
        pub source_release: i64,
        pub build_release: i64,
        pub architecture: String,
        pub summary: String,
        pub description: String,
        pub source_id: String,
        pub homepage: String,
        pub uri: Option<String>,
        pub hash: Option<String>,
        pub download_size: Option<i64>,
    }

    #[derive(FromRow)]
    pub struct License {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub license: String,
    }

    #[derive(FromRow)]
    pub struct Dependency {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub dependency: Decoder<crate::Dependency>,
    }

    #[derive(FromRow)]
    pub struct Provider {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub provider: Decoder<crate::Provider>,
    }

    #[derive(FromRow)]
    pub struct ProviderPackage {
        pub package: Decoder<package::Id>,
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use stone::read::Payload;

    use super::*;

    #[tokio::test]
    async fn create_insert_select() {
        let database =
            Database::connect(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();

        let bash_completion = include_bytes!("../../../test/bash-completion-2.11-1-1-x86_64.stone");

        let mut stone = stone::read_bytes(bash_completion).unwrap();

        let payloads = stone
            .payloads()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let meta_payload = payloads.iter().find_map(Payload::meta).unwrap();
        let meta = Meta::from_stone_payload(meta_payload).unwrap();

        let id = package::Id::from("test".to_string());

        database.add(id.clone(), meta.clone()).await.unwrap();

        assert_eq!(&meta.name, &"bash-completion".to_string().into());

        remove(id.clone(), &mut database.pool.acquire().await.unwrap())
            .await
            .unwrap();

        let result = database.get(&id).await;

        assert!(result.is_err());

        // Test wipe
        database.add(id.clone(), meta.clone()).await.unwrap();
        database.wipe().await.unwrap();
        let result = database.get(&id).await;
        assert!(result.is_err());
    }
}
