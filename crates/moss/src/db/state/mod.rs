// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Acquire, Executor, Pool, Sqlite};
use thiserror::Error;

use crate::db::Encoding;
use crate::state::{self, Id, Selection};
use crate::{Installation, State};

#[derive(Debug)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(installation: &Installation) -> Result<Self, Error> {
        let path = installation.db_path("state");

        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .read_only(installation.read_only())
            .foreign_keys(true);

        Self::connect(options).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self, Error> {
        let pool = sqlx::SqlitePool::connect_with(options).await?;

        sqlx::migrate!("src/db/state/migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn list_ids(&self) -> Result<Vec<(Id, DateTime<Utc>)>, Error> {
        let states = sqlx::query_as::<_, encoding::Created>(
            "
            SELECT id, created
            FROM state;
            ",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(states
            .into_iter()
            .map(|state| (state.id.0, state.created))
            .collect())
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
        let selections_query = sqlx::query_as::<_, encoding::Selection>(
            "
            SELECT package_id,
                   explicit,
                   reason
            FROM state_selections
            WHERE state_id = ?;
            ",
        )
        .bind(id.encode());

        let (state, selections_rows) = futures::try_join!(
            state_query.fetch_one(&self.pool),
            selections_query.fetch_all(&self.pool)
        )?;

        let selections = selections_rows
            .into_iter()
            .map(|row| Selection {
                package: row.package_id.0,
                explicit: row.explicit,
                reason: row.reason,
            })
            .collect();

        Ok(State {
            id: state.id.0,
            summary: state.summary,
            description: state.description,
            selections,
            created: state.created,
            kind: state.kind.0,
        })
    }

    pub async fn add(
        &self,
        selections: &[Selection],
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
        .bind(state::Kind::Transaction.encode())
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
                        b.push_bind(id.0.encode())
                            .push_bind(selection.package.encode())
                            .push_bind(selection.explicit)
                            .push_bind(selection.reason.as_ref());
                    })
                    .build(),
                )
                .await?;
        }

        transaction.commit().await?;

        let state = self.get(&id.0).await?;

        Ok(state)
    }

    pub async fn remove(&self, state: &state::Id) -> Result<(), Error> {
        self.batch_remove(Some(state)).await
    }

    pub async fn batch_remove(
        &self,
        states: impl IntoIterator<Item = &state::Id>,
    ) -> Result<(), Error> {
        let mut query = sqlx::QueryBuilder::new(
            "
            DELETE FROM state
            WHERE id IN ( 
            ",
        );

        let mut separated = query.separated(", ");
        states.into_iter().for_each(|id| {
            separated.push_bind(id.encode());
        });
        separated.push_unseparated(");");

        query.build().execute(&self.pool).await?;

        Ok(())
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

    use super::{state, Id};
    use crate::{db::Decoder, package};

    #[derive(FromRow)]
    pub struct Created {
        pub id: Decoder<Id>,
        pub created: DateTime<Utc>,
    }

    #[derive(FromRow)]
    pub struct State {
        pub id: Decoder<Id>,
        #[sqlx(rename = "type")]
        pub kind: Decoder<state::Kind>,
        pub created: DateTime<Utc>,
        pub summary: Option<String>,
        pub description: Option<String>,
    }

    #[derive(FromRow)]
    pub struct StateId {
        pub id: Decoder<Id>,
    }

    #[derive(FromRow)]
    pub struct Selection {
        pub package_id: Decoder<package::Id>,
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

    #[tokio::test]
    async fn create_insert_select() {
        let database =
            Database::connect(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();

        let selections = vec![
            Selection::explicit(package::Id::from("pkg a".to_string())),
            Selection::explicit(package::Id::from("pkg a".to_string())),
            Selection::explicit(package::Id::from("pkg a".to_string())),
        ];

        let state = database
            .add(
                &selections,
                Some("test".to_string()),
                Some("test".to_string()),
            )
            .await
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
