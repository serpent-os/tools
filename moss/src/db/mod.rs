// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::SqliteConnection;
use thiserror::Error;

pub mod layout;
pub mod meta;
pub mod state;

#[derive(Clone)]
struct Connection(Arc<Mutex<SqliteConnection>>);

impl Connection {
    fn new(connection: SqliteConnection) -> Self {
        Self(Arc::new(Mutex::new(connection)))
    }

    fn exec<T>(&self, f: impl FnOnce(&mut SqliteConnection) -> T) -> T {
        let mut _guard = self.0.lock().expect("mutex guard");
        f(&mut _guard)
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}

pub struct Timestamp(pub DateTime<Utc>);

impl TryFrom<i64> for Timestamp {
    type Error = Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(Self(
            NaiveDateTime::from_timestamp_opt(value, 0)
                .ok_or(Error::InvalidTimestamp(value))?
                .and_utc(),
        ))
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Row not found")]
    RowNotFound,
    #[error("failed to decode layout entry")]
    LayoutEntryDecode,
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("diesel")]
    Diesel(#[from] diesel::result::Error),
    #[error("diesel connection")]
    Connection(#[from] diesel::ConnectionError),
    #[error("diesel migration")]
    Migration(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}
