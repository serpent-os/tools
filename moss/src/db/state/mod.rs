// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Acquire, Executor};
use thiserror::Error;

use super::Pool;
use crate::state::{self, Id, Selection};
use crate::{runtime, Installation, State};

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool,
}

impl Database {
    pub fn new(installation: &Installation) -> Result<Self, Error> {
        let path = installation.db_path("state");

        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .read_only(installation.read_only())
            .foreign_keys(true);

        Self::connect(options)
    }

    fn connect(options: SqliteConnectOptions) -> Result<Self, Error> {
        runtime::block_on(async {
            let pool = sqlx::SqlitePool::connect_with(options).await?;
            sqlx::migrate!("src/db/state/migrations").run(&pool).await?;
            Ok(pool)
        })
        .map(|pool| Self { pool: Pool::new(pool) })
    }

    pub fn list_ids(&self) -> Result<Vec<(Id, DateTime<Utc>)>, Error> {
        self.pool.exec(|pool| async move {
            let states = sqlx::query_as::<_, encoding::Created>(
                "
                SELECT id, created
                FROM state;
                ",
            )
            .fetch_all(&pool)
            .await?;

            Ok(states.into_iter().map(|state| (state.id, state.created)).collect())
        })
    }

    pub fn get(&self, id: &Id) -> Result<State, Error> {
        self.pool.exec(|pool| async move {
            let state_query = sqlx::query_as::<_, encoding::State>(
                "
                SELECT id, type, created, summary, description
                FROM state
                WHERE id = ?;
                ",
            )
            .bind(i64::from(*id));
            let selections_query = sqlx::query_as::<_, encoding::Selection>(
                "
                SELECT package_id,
                       explicit,
                       reason
                FROM state_selections
                WHERE state_id = ?;
                ",
            )
            .bind(i64::from(*id));

            let state = state_query.fetch_one(&pool).await?;
            let selections_rows = selections_query.fetch_all(&pool).await?;

            let selections = selections_rows
                .into_iter()
                .map(|row| Selection {
                    package: row.package_id,
                    explicit: row.explicit,
                    reason: row.reason,
                })
                .collect();

            Ok(State {
                id: state.id,
                summary: state.summary,
                description: state.description,
                selections,
                created: state.created,
                kind: state.kind,
            })
        })
    }

    pub fn add(
        &self,
        selections: &[Selection],
        summary: Option<String>,
        description: Option<String>,
    ) -> Result<State, Error> {
        self.pool
            .exec(|pool| async move {
                let mut transaction = pool.begin().await?;

                let encoding::StateId { id } = sqlx::query_as::<_, encoding::StateId>(
                    "
                    INSERT INTO state (type, summary, description)
                    VALUES (?, ?, ?)
                    RETURNING id;
                    ",
                )
                .bind(state::Kind::Transaction.to_string())
                .bind(summary)
                .bind(description)
                .fetch_one(transaction.acquire().await?)
                .await?;

                if !selections.is_empty() {
                    transaction
                        .execute(
                            sqlx::QueryBuilder::new(
                                "
                                INSERT INTO state_selections (state_id, package_id, explicit, reason)
                                ",
                            )
                            .push_values(selections, |mut b, selection| {
                                b.push_bind(i64::from(id))
                                    .push_bind(selection.package.to_string())
                                    .push_bind(selection.explicit)
                                    .push_bind(selection.reason.as_ref());
                            })
                            .build(),
                        )
                        .await?;
                }

                transaction.commit().await?;

                Ok(id)
            })
            .and_then(|id| self.get(&id))
    }

    pub fn remove(&self, state: &state::Id) -> Result<(), Error> {
        self.batch_remove(Some(state))
    }

    pub fn batch_remove<'a>(&self, states: impl IntoIterator<Item = &'a state::Id>) -> Result<(), Error> {
        self.pool.exec(|pool| async move {
            let mut query = sqlx::QueryBuilder::new(
                "
                DELETE FROM state
                WHERE id IN ( 
                ",
            );

            let mut separated = query.separated(", ");
            states.into_iter().for_each(|id| {
                separated.push_bind(i64::from(*id));
            });
            separated.push_unseparated(");");

            query.build().execute(&pool).await?;

            Ok(())
        })
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
    use chrono::{DateTime, Utc};
    use sqlx::FromRow;

    use crate::package;
    use crate::state::{self, Id};

    #[derive(FromRow)]
    pub struct Created {
        #[sqlx(try_from = "i64")]
        pub id: Id,
        pub created: DateTime<Utc>,
    }

    #[derive(FromRow)]
    pub struct State {
        #[sqlx(try_from = "i64")]
        pub id: Id,
        #[sqlx(rename = "type", try_from = "&'a str")]
        pub kind: state::Kind,
        pub created: DateTime<Utc>,
        pub summary: Option<String>,
        pub description: Option<String>,
    }

    #[derive(FromRow)]
    pub struct StateId {
        #[sqlx(try_from = "i64")]
        pub id: Id,
    }

    #[derive(FromRow)]
    pub struct Selection {
        #[sqlx(try_from = "String")]
        pub package_id: package::Id,
        pub explicit: bool,
        pub reason: Option<String>,
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use chrono::Utc;

    use super::*;
    use crate::package;

    async fn create_insert_select() {
        let _guard = runtime::init();

        let database = Database::connect(SqliteConnectOptions::from_str("sqlite::memory:").unwrap()).unwrap();

        let selections = vec![
            Selection::explicit(package::Id::from("pkg a".to_string())),
            Selection::explicit(package::Id::from("pkg a".to_string())),
            Selection::explicit(package::Id::from("pkg a".to_string())),
        ];

        let state = database
            .add(&selections, Some("test".to_string()), Some("test".to_string()))
            .unwrap();

        // First record
        assert_eq!(i64::from(state.id), 1);

        // Check created
        let elapsed = Utc::now().signed_duration_since(state.created);
        assert!(elapsed.num_seconds() == 0);
        assert!(!elapsed.is_zero());

        assert_eq!(state.summary.as_deref(), Some("test"));
        assert_eq!(state.description.as_deref(), Some("test"));

        assert_eq!(state.selections, selections);
    }
}
