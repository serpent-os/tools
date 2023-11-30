// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Pool, Sqlite};
use stone::payload::{self};
use thiserror::Error;

use crate::package;
use crate::Installation;

use super::Encoding;

#[derive(Debug)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(installation: &Installation) -> Result<Self, Error> {
        let path = installation.db_path("layout");

        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .read_only(installation.read_only())
            .foreign_keys(true);

        Self::connect(options).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self, Error> {
        let pool = sqlx::SqlitePool::connect_with(options).await?;

        sqlx::migrate!("src/db/layout/migrations")
            .run(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn all(&self) -> Result<Vec<(package::Id, payload::Layout)>, Error> {
        let layouts = sqlx::query_as::<_, encoding::Layout>(
            "
            SELECT package_id,
                   uid,
                   gid,
                   mode,
                   tag,
                   entry_type,
                   entry_value1,
                   entry_value2
            FROM layout;
            ",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(layouts
            .into_iter()
            .filter_map(|layout| {
                let encoding::Layout {
                    package_id,
                    uid,
                    gid,
                    mode,
                    tag,
                    entry_type,
                    entry_value1,
                    entry_value2,
                } = layout;

                let entry = encoding::decode_entry(entry_type, entry_value1, entry_value2)?;

                Some((
                    package_id.0,
                    payload::Layout {
                        uid,
                        gid,
                        mode,
                        tag,
                        entry,
                    },
                ))
            })
            .collect())
    }

    pub async fn file_hashes(&self) -> Result<HashSet<String>, Error> {
        let layouts = sqlx::query_as::<_, (String,)>(
            "
            SELECT DISTINCT entry_value1
            FROM layout
            WHERE entry_type = 'regular';
            ",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(layouts
            .into_iter()
            .filter_map(|(hash,)| hash.parse::<u128>().ok().map(|hash| format!("{hash:02x}")))
            .collect())
    }

    pub async fn add(&self, package: package::Id, layout: payload::Layout) -> Result<(), Error> {
        self.batch_add(vec![(package, layout)]).await
    }

    pub async fn batch_add(
        &self,
        layouts: Vec<(package::Id, payload::Layout)>,
    ) -> Result<(), Error> {
        sqlx::QueryBuilder::new(
            "
            INSERT INTO layout 
            (
                package_id,
                uid,
                gid,
                mode,
                tag,
                entry_type,
                entry_value1,
                entry_value2
            )
            ",
        )
        .push_values(layouts, |mut b, (id, layout)| {
            let payload::Layout {
                uid,
                gid,
                mode,
                tag,
                entry,
            } = layout;

            let (entry_type, entry_value1, entry_value2) = encoding::encode_entry(entry);

            b.push_bind(id.encode().to_owned())
                .push_bind(uid)
                .push_bind(gid)
                .push_bind(mode)
                .push_bind(tag)
                .push_bind(entry_type)
                .push_bind(entry_value1)
                .push_bind(entry_value2);
        })
        .build()
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove(&self, package: &package::Id) -> Result<(), Error> {
        self.batch_remove(Some(package)).await
    }

    pub async fn batch_remove(
        &self,
        packages: impl IntoIterator<Item = &package::Id>,
    ) -> Result<(), Error> {
        let mut query = sqlx::QueryBuilder::new(
            "
            DELETE FROM layout
            WHERE package_id IN ( 
            ",
        );

        let mut separated = query.separated(", ");
        packages.into_iter().for_each(|pkg| {
            separated.push_bind(pkg.encode());
        });
        separated.push_unseparated(");");

        query.build().execute(&self.pool).await?;

        Ok(())
    }

    /// Retrieve all entries for a given package by ID
    pub async fn query(&self, package: &package::Id) -> Result<Vec<payload::Layout>, Error> {
        let query = sqlx::query_as::<_, encoding::Layout>(
            "SELECT package_id,
                   uid,
                   gid,
                   mode,
                   tag,
                   entry_type,
                   entry_value1,
                   entry_value2
            FROM layout WHERE package_id = ?",
        )
        .bind(package.encode());

        let layouts = query.fetch_all(&self.pool).await?;

        Ok(layouts
            .into_iter()
            .filter_map(|layout| {
                let encoding::Layout {
                    package_id,
                    uid,
                    gid,
                    mode,
                    tag,
                    entry_type,
                    entry_value1,
                    entry_value2,
                } = layout;

                let entry = encoding::decode_entry(entry_type, entry_value1, entry_value2)?;

                Some(payload::Layout {
                    uid,
                    gid,
                    mode,
                    tag,
                    entry,
                })
            })
            .collect())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("sqlx")]
    Sqlx(#[from] sqlx::Error),
    #[error("sqlx migration")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

mod encoding {
    use sqlx::FromRow;
    use stone::payload;

    use crate::{db::Decoder, package};

    #[derive(FromRow)]
    pub struct Layout {
        pub package_id: Decoder<package::Id>,
        pub uid: u32,
        pub gid: u32,
        pub mode: u32,
        pub tag: u32,
        pub entry_type: String,
        pub entry_value1: Option<String>,
        pub entry_value2: Option<String>,
    }

    pub fn decode_entry(
        entry_type: String,
        entry_value1: Option<String>,
        entry_value2: Option<String>,
    ) -> Option<payload::layout::Entry> {
        use payload::layout::Entry;

        match entry_type.as_str() {
            "regular" => {
                let hash = entry_value1?.parse::<u128>().ok()?;
                let name = entry_value2?;

                Some(Entry::Regular(hash, name))
            }
            "symlink" => Some(Entry::Symlink(entry_value1?, entry_value2?)),
            "directory" => Some(Entry::Directory(entry_value1?)),
            "character-device" => Some(Entry::CharacterDevice(entry_value1?)),
            "block-device" => Some(Entry::BlockDevice(entry_value1?)),
            "fifo" => Some(Entry::Fifo(entry_value1?)),
            "socket" => Some(Entry::Socket(entry_value1?)),
            _ => None,
        }
    }

    pub fn encode_entry(
        entry: payload::layout::Entry,
    ) -> (&'static str, Option<String>, Option<String>) {
        use payload::layout::Entry;

        match entry {
            Entry::Regular(hash, name) => ("regular", Some(hash.to_string()), Some(name)),
            Entry::Symlink(a, b) => ("symlink", Some(a), Some(b)),
            Entry::Directory(name) => ("directory", Some(name), None),
            Entry::CharacterDevice(name) => ("character-device", Some(name), None),
            Entry::BlockDevice(name) => ("block-device", Some(name), None),
            Entry::Fifo(name) => ("fifo", Some(name), None),
            Entry::Socket(name) => ("socket", Some(name), None),
        }
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use stone::read::PayloadKind;

    use super::*;

    #[tokio::test]
    async fn create_insert_select() {
        let database =
            Database::connect(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();

        let bash_completion =
            include_bytes!("../../../../../test/bash-completion-2.11-1-1-x86_64.stone");

        let mut stone = stone::read_bytes(bash_completion).unwrap();

        let payloads = stone
            .payloads()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let layouts = payloads
            .iter()
            .filter_map(PayloadKind::layout)
            .flat_map(|p| &p.body)
            .cloned()
            .map(|layout| (package::Id::from("test".to_string()), layout))
            .collect::<Vec<_>>();

        let count = layouts.len();

        database.batch_add(layouts).await.unwrap();

        let all = database.all().await.unwrap();

        assert_eq!(count, all.len());
    }
}
