// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Signal handling

use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet};
use thiserror::Error;

pub use nix::sys::signal::Signal;

/// Ignore the provided signals until [`Guard`] is dropped
pub fn ignore(signals: impl IntoIterator<Item = Signal>) -> Result<Guard, Error> {
    Ok(Guard(
        signals
            .into_iter()
            .map(|signal| unsafe {
                let action = sigaction(
                    signal,
                    &SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty()),
                )
                .map_err(Error::Ignore)?;

                Ok(PrevHandler { signal, action })
            })
            .collect::<Result<_, Error>>()?,
    ))
}

/// A guard which restores the previous signal
/// handlers when dropped
pub struct Guard(Vec<PrevHandler>);

impl Drop for Guard {
    fn drop(&mut self) {
        for PrevHandler { signal, action } in &self.0 {
            unsafe {
                let _ = sigaction(*signal, action);
            };
        }
    }
}

struct PrevHandler {
    signal: Signal,
    action: SigAction,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("ignore signal")]
    Ignore(#[source] nix::Error),
}
