// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{future::Future, io};

use tokio::runtime;

pub struct Runtime(runtime::Runtime);

impl Runtime {
    pub fn new() -> io::Result<Self> {
        Ok(Self(
            runtime::Builder::new_multi_thread().enable_all().build()?,
        ))
    }

    pub fn block_on<T, F>(&self, task: F) -> T
    where
        F: Future<Output = T>,
    {
        self.0.block_on(task)
    }
}
