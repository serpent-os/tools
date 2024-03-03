// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    future::Future,
    io,
    sync::{OnceLock, RwLock},
};

use tokio::runtime::{self, Handle};

static RUNTIME: OnceLock<RwLock<Option<Runtime>>> = OnceLock::new();

pub fn init() -> Guard {
    let lock = RUNTIME.get_or_init(Default::default);
    *lock.write().unwrap() = Some(Runtime::new().expect("build runtime"));
    Guard
}

fn destroy() {
    let rt = RUNTIME
        .get()
        .unwrap()
        .write()
        .unwrap()
        .take()
        .expect("runtime initialized");
    drop(rt);
}

/// Drop the Guard to drop the runtime!
#[must_use = "runtime is dropped with guard"]
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        destroy()
    }
}

struct Runtime(runtime::Runtime);

impl Runtime {
    fn new() -> io::Result<Self> {
        Ok(Self(runtime::Builder::new_current_thread().enable_all().build()?))
    }
}

/// Run the provided future on the current runtime.
pub fn block_on<T, F>(task: F) -> T
where
    F: Future<Output = T>,
{
    let _guard = RUNTIME.get().unwrap().read().unwrap();
    let rt = _guard.as_ref().expect("runtime initialized");
    rt.0.block_on(task)
}

/// Runs the provided function on an executor dedicated to blocking.
pub async fn unblock<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let handle = Handle::current();
    handle.spawn_blocking(f).await.expect("spawn blocking")
}
