// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::fmt;

use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Acquire, Executor, Pool, Sqlite};
use thiserror::Error;

use crate::db::Encoding;
use crate::registry::package;
use crate::Installation;

/// Unique identifier for [`State`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id(i64);

impl From<i64> for Id {
    fn from(id: i64) -> Self {
        Id(id)
    }
}

impl From<Id> for i64 {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// State types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    /// Automatically constructed state
    Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// Unique identifier for this state
    pub id: Id,
    /// Quick summary for the state (optional)
    pub summary: Option<String>,
    /// Description for the state (optional)
    pub description: Option<String>,
    /// Package IDs / selections in this state
    pub packages: Vec<package::Id>,
    /// Creation timestamp
    pub created: Timestamp,
    /// Relevant type for this State
    pub kind: Kind,
}

// TODO: Add crate timestamp type that can be reused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp(DateTime<Utc>);

#[derive(Debug)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(installation: &Installation) -> Result<Self, Error> {
        let path = installation.db_path("state");

        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .read_only(installation.read_only());

        Self::connect(options).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self, Error> {
        let pool = sqlx::SqlitePool::connect_with(options).await?;

        sqlx::migrate!("src/db/state/migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn get(&self, id: &Id) -> Result<State, Error> {
        let state_query = sqlx::query_as::<_, encoding::State>(
            "
            SELECT id, type, created, summary, description
            FROM state
            WHERE id = ?;
            ",
        )
        .bind(id.encode());
        let packages_query = sqlx::query_as::<_, encoding::Package>(
            "
            SELECT package_id
            FROM packages
            WHERE state_id = ?;
            ",
        )
        .bind(id.encode());

        let (state, package_rows) = futures::try_join!(
            state_query.fetch_one(&self.pool),
            packages_query.fetch_all(&self.pool)
        )?;

        let packages = package_rows
            .into_iter()
            .map(|row| row.package_id.0)
            .collect();

        Ok(State {
            id: state.id.0,
            summary: state.summary,
            description: state.description,
            packages,
            created: Timestamp(state.created),
            kind: state.kind.0,
        })
    }

    pub async fn add(
        &self,
        packages: &[package::Id],
        summary: Option<String>,
        description: Option<String>,
    ) -> Result<State, Error> {
        let mut transaction = self.pool.begin().await?;

        let encoding::StateId { id } = sqlx::query_as::<_, encoding::StateId>(
            "
            INSERT INTO state (type, summary, description)
            VALUES (?, ?, ?)
            RETURNING id;
            ",
        )
        .bind(Kind::Transaction.encode())
        .bind(summary)
        .bind(description)
        .fetch_one(transaction.acquire().await?)
        .await?;

        transaction
            .execute(
                sqlx::QueryBuilder::new(
                    "
                    INSERT INTO packages (state_id, package_id, reason)
                    ",
                )
                .push_values(packages, |mut b, package| {
                    b.push_bind(id.0.encode())
                        .push_bind(package.clone().encode())
                        .push_bind(Option::<String>::None);
                })
                .build(),
            )
            .await?;

        transaction.commit().await?;

        let state = self.get(&id.0).await?;

        Ok(state)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

mod encoding {
    use std::convert::Infallible;

    use chrono::{DateTime, Utc};
    use sqlx::FromRow;
    use thiserror::Error;

    use super::{Id, Kind};
    use crate::{
        db::{Decoder, Encoding},
        registry::package,
    };

    #[derive(FromRow)]
    pub struct State {
        pub id: Decoder<Id>,
        #[sqlx(rename = "type")]
        pub kind: Decoder<Kind>,
        pub created: DateTime<Utc>,
        pub summary: Option<String>,
        pub description: Option<String>,
    }

    #[derive(FromRow)]
    pub struct StateId {
        pub id: Decoder<Id>,
    }

    #[derive(FromRow)]
    pub struct Package {
        pub package_id: Decoder<package::Id>,
    }

    impl Encoding for Id {
        type Encoded = i64;
        type Error = Infallible;

        fn decode(value: i64) -> Result<Self, Self::Error> {
            Ok(Self(value))
        }

        fn encode(self) -> i64 {
            self.0
        }
    }

    impl Encoding for Kind {
        type Encoded = String;
        type Error = DecodeKindError;

        fn decode(value: String) -> Result<Self, Self::Error> {
            match value.as_str() {
                "transaction" => Ok(Self::Transaction),
                _ => Err(DecodeKindError(value)),
            }
        }

        fn encode(self) -> Self::Encoded {
            match self {
                Kind::Transaction => "transaction".into(),
            }
        }
    }

    #[derive(Debug, Error)]
    #[error("Invalid state type: {0}")]
    pub struct DecodeKindError(String);
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use chrono::Utc;
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn create_insert_select() {
        block_on(async {
            let database =
                Database::connect(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                    .await
                    .unwrap();

            let packages = vec![
                package::Id::from("pkg a".to_string()),
                package::Id::from("pkg b".to_string()),
                package::Id::from("pkg c".to_string()),
            ];

            let state = database
                .add(
                    &packages,
                    Some("test".to_string()),
                    Some("test".to_string()),
                )
                .await
                .unwrap();

            // First record
            assert_eq!(state.id.0, 1);

            // Check created
            let elapsed = Utc::now().signed_duration_since(&state.created.0);
            assert!(elapsed.num_seconds() == 0);
            assert!(!elapsed.is_zero());

            assert_eq!(state.summary.as_deref(), Some("test"));
            assert_eq!(state.description.as_deref(), Some("test"));

            assert_eq!(state.packages, packages);
        });
    }
}
