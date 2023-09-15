// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use sqlx::SqliteConnection;
use sqlx::{sqlite::SqliteConnectOptions, Acquire, Pool, Sqlite};
use stone::payload;
use thiserror::Error;

use crate::db::Encoding;
use crate::{dependency, registry::package, Dependency, Provider};

#[derive(Debug, Clone)]
pub struct Entry {
    /// Primary key in the db *is* the package ID
    pub package: package::Id,
    /// Package name
    pub name: String,
    /// Human readable version identifier
    pub version_identifier: String,
    /// Package release as set in stone.yml
    pub source_release: u64,
    /// Build machinery specific build release
    pub build_release: u64,
    /// Architecture this was built for
    pub architecture: String,
    /// Brief one line summary of the package
    pub summary: String,
    /// Description of the package
    pub description: String,
    /// The source-grouping ID
    pub source_id: String,
    /// Where'd we find this guy..
    pub homepage: String,
    /// Licenses this is available under
    pub licenses: Vec<String>,
    /// All dependencies
    pub dependencies: Vec<Dependency>,
    /// All providers, including name()
    pub providers: Vec<Provider>,
    /// If relevant: uri to fetch from
    pub uri: Option<String>,
    /// If relevant: hash for the download
    pub hash: Option<String>,
    /// How big is this package in the repo..?
    pub download_size: Option<u64>,
}

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

    pub async fn get(&self, package: &package::Id) -> Result<Entry, Error> {
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
            SELECT license
            FROM meta_licenses
            WHERE package = ?;
            ",
        )
        .bind(package.clone().encode());

        let dependencies_query = sqlx::query_as::<_, encoding::Dependency>(
            "
            SELECT dependency
            FROM meta_dependencies
            WHERE package = ?;
            ",
        )
        .bind(package.clone().encode());

        let providers_query = sqlx::query_as::<_, encoding::Provider>(
            "
            SELECT provider
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

        Ok(Entry {
            package: entry.package.0,
            name: entry.name,
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

    // TODO: Make more safe, this module shouldn't deal w/ metadata. Caller should convert to Entry
    pub async fn load_stone_metadata(&self, metadata: &[payload::Meta]) -> Result<Entry, Error> {
        let entry = build_entry(metadata)?;

        let mut transaction = self.pool.begin().await?;

        // Remove package (other tables cascade)
        remove(entry.package.clone(), transaction.acquire().await?).await?;

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
        .bind(entry.package.clone().encode())
        .bind(entry.name)
        .bind(entry.version_identifier)
        .bind(entry.source_release as i64)
        .bind(entry.build_release as i64)
        .bind(entry.architecture)
        .bind(entry.summary)
        .bind(entry.description)
        .bind(entry.source_id)
        .bind(entry.homepage)
        .bind(entry.uri)
        .bind(entry.hash)
        .bind(entry.download_size.map(|i| i as i64))
        .execute(transaction.acquire().await?)
        .await?;

        // Licenses
        if !entry.licenses.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_licenses (package, license)
                ",
            )
            .push_values(entry.licenses, |mut b, license| {
                b.push_bind(entry.package.clone().encode())
                    .push_bind(license);
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        // Dependencies
        if !entry.dependencies.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_dependencies (package, dependency)
                ",
            )
            .push_values(entry.dependencies, |mut b, dependency| {
                b.push_bind(entry.package.clone().encode())
                    .push_bind(dependency.encode());
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        // Providers
        if !entry.providers.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_providers (package, provider)
                ",
            )
            .push_values(entry.providers, |mut b, provider| {
                b.push_bind(entry.package.clone().encode())
                    .push_bind(provider.encode());
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        transaction.commit().await?;

        self.get(&entry.package).await
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

fn build_entry(metadata: &[payload::Meta]) -> Result<Entry, Error> {
    let name = required_meta_string(metadata, payload::meta::Tag::Name)?;
    let version_identifier = required_meta_string(metadata, payload::meta::Tag::Version)?;
    let source_release = required_meta_u64(metadata, payload::meta::Tag::Release)?;
    let build_release = required_meta_u64(metadata, payload::meta::Tag::BuildRelease)?;
    let architecture = required_meta_string(metadata, payload::meta::Tag::Architecture)?;
    let summary = required_meta_string(metadata, payload::meta::Tag::Summary)?;
    let description = required_meta_string(metadata, payload::meta::Tag::Description)?;
    let source_id = required_meta_string(metadata, payload::meta::Tag::SourceID)?;
    let homepage = required_meta_string(metadata, payload::meta::Tag::Homepage)?;
    let uri = required_meta_string(metadata, payload::meta::Tag::PackageURI).ok();
    let hash = required_meta_string(metadata, payload::meta::Tag::PackageHash).ok();
    let download_size = required_meta_u64(metadata, payload::meta::Tag::PackageSize).ok();

    let package = package::Id::from(hash.as_ref().unwrap_or(&name).clone());

    let licenses = metadata
        .iter()
        .filter_map(|meta| meta_string(meta, payload::meta::Tag::License))
        .collect();
    let dependencies = metadata
        .iter()
        .filter_map(|meta| meta_dependency(meta))
        .collect();
    let providers = metadata
        .iter()
        .filter_map(|meta| meta_provider(meta))
        // Add package name as provider
        .chain(Some(Provider {
            kind: dependency::Kind::PackageName,
            name: name.clone(),
        }))
        .collect();

    Ok(Entry {
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
        licenses,
        dependencies,
        providers,
        uri,
        hash,
        download_size,
    })
}

fn required_meta_string(
    metadata: &[payload::Meta],
    tag: payload::meta::Tag,
) -> Result<String, Error> {
    metadata
        .iter()
        .find_map(|meta| meta_string(meta, tag))
        .ok_or(Error::MissingMetaField(tag))
}

fn required_meta_u64(metadata: &[payload::Meta], tag: payload::meta::Tag) -> Result<u64, Error> {
    metadata
        .iter()
        .find_map(|meta| meta_u64(meta, tag))
        .ok_or(Error::MissingMetaField(tag))
}

fn meta_u64(meta: &payload::Meta, tag: payload::meta::Tag) -> Option<u64> {
    if meta.tag == tag {
        Some(match meta.kind {
            payload::meta::Kind::Int8(i) => i as _,
            payload::meta::Kind::Uint8(i) => i as _,
            payload::meta::Kind::Int16(i) => i as _,
            payload::meta::Kind::Uint16(i) => i as _,
            payload::meta::Kind::Int32(i) => i as _,
            payload::meta::Kind::Uint32(i) => i as _,
            payload::meta::Kind::Int64(i) => i as _,
            payload::meta::Kind::Uint64(i) => i,
            _ => return None,
        })
    } else {
        None
    }
}

fn meta_string(meta: &payload::Meta, tag: payload::meta::Tag) -> Option<String> {
    match (meta.tag, &meta.kind) {
        (meta_tag, payload::meta::Kind::String(value)) if meta_tag == tag => Some(value.clone()),
        _ => None,
    }
}

fn meta_dependency(meta: &payload::Meta) -> Option<Dependency> {
    if let payload::meta::Kind::Dependency(kind, name) = meta.kind.clone() {
        Some(Dependency {
            kind: dependency::Kind::from(kind),
            name,
        })
    } else {
        None
    }
}

fn meta_provider(meta: &payload::Meta) -> Option<Provider> {
    if let payload::meta::Kind::Provider(kind, name) = meta.kind.clone() {
        Some(Provider {
            kind: dependency::Kind::from(kind),
            name,
        })
    } else {
        None
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing metadata field: {0:?}")]
    MissingMetaField(payload::meta::Tag),
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

mod encoding {
    use sqlx::FromRow;

    use crate::db::Decoder;
    use crate::registry::package;

    #[derive(FromRow)]
    pub struct Entry {
        pub package: Decoder<package::Id>,
        pub name: String,
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
        pub license: String,
    }

    #[derive(FromRow)]
    pub struct Dependency {
        pub dependency: Decoder<crate::Dependency>,
    }

    #[derive(FromRow)]
    pub struct Provider {
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
        let meta = payloads
            .iter()
            .filter_map(Payload::meta)
            .flatten()
            .cloned()
            .collect::<Vec<_>>();

        let package = package::Id::from("test".to_string());

        let entry = database.load_stone_metadata(&meta).await.unwrap();

        assert_eq!(entry.name, "bash-completion".to_string());

        remove(package.clone(), &mut database.pool.acquire().await.unwrap())
            .await
            .unwrap();

        let result = database.get(&package).await;

        assert!(result.is_err());

        // Test wipe
        database.load_stone_metadata(&meta).await.unwrap();
        database.wipe().await.unwrap();
        let result = database.get(&package).await;
        assert!(result.is_err());
    }
}
