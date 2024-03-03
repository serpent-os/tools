// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use sqlx::Sqlite;

use crate::runtime;

pub mod layout;
pub mod meta;
pub mod state;

#[derive(Debug, Clone)]
struct Pool(Arc<Mutex<sqlx::Pool<Sqlite>>>);

impl Pool {
    fn new(pool: sqlx::Pool<Sqlite>) -> Self {
        Self(Arc::new(Mutex::new(pool)))
    }

    fn exec<F, T>(&self, f: impl FnOnce(sqlx::Pool<Sqlite>) -> F) -> T
    where
        F: Future<Output = T>,
    {
        let _guard = self.0.lock().expect("mutex guard");
        let pool = _guard.clone();
        runtime::block_on(f(pool))
    }
}
