// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{future::Future, io, thread, time::Duration};

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

    pub fn destroy(self) {
        drop(self);
        // We want to ensure no threads exist before
        // cloning into container. Sometimes a deadlock
        // occurs which appears related to a race condition
        // from some thread artifacts still existing. Adding
        // this delay allows things to get cleaned up.
        // NOTE: This appears to reliably fix the problem,
        // I ran boulder 100 times w/ and w/out this delay
        // and the deadlock never occured w/ it, but w/out
        // it occured within 10 attempts.
        thread::sleep(Duration::from_millis(50));
    }
}
