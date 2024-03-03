// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Operations that happen post-blit (primarily, triggers within container)
//! Note that we support transaction scope and system scope triggers, invoked
//! before `/usr` is activated and after, respectively.
//!
//! Note that currently we only load from `/usr/share/moss/triggers/{tx,sys.d}/*.yaml`
//! and do not yet support local triggers
use std::{
    path::{Path, PathBuf},
    process,
};

use container::Container;
use itertools::Itertools;
use serde::Deserialize;
use thiserror::Error;
use triggers::{
    format::{Handler, Trigger},
    TriggerCommand,
};

use crate::Installation;

use super::PendingFile;

/// Transaction triggers
/// These are loaded from `/usr/share/moss/triggers/tx.d/*.yaml`
#[derive(Deserialize, Debug)]
struct TransactionTrigger(Trigger);

impl config::Config for TransactionTrigger {
    fn domain() -> String {
        "tx".into()
    }
}

#[derive(Deserialize, Debug)]
struct SystemTrigger(Trigger);

impl config::Config for SystemTrigger {
    fn domain() -> String {
        "sys".into()
    }
}

/// Defines the scope of triggers
#[derive(Clone, Copy, Debug)]
pub(super) enum TriggerScope<'a> {
    Transaction(&'a Installation, &'a super::Scope),
    System(&'a Installation, &'a super::Scope),
}

impl<'a> TriggerScope<'a> {
    // Determine the correct root directory
    fn root_dir(&self) -> PathBuf {
        match self {
            TriggerScope::Transaction(install, scope) => match scope {
                super::Scope::Stateful => install.staging_dir().clone(),
                super::Scope::Ephemeral { blit_root } => blit_root.clone(),
            },
            TriggerScope::System(install, scope) => match scope {
                super::Scope::Stateful => install.root.clone(),
                super::Scope::Ephemeral { blit_root } => blit_root.clone(),
            },
        }
    }

    /// Join "host" paths, outside the staging filesystem. Ensure no sandbox break for ephemeral
    fn host_path(&self, path: impl AsRef<Path>) -> PathBuf {
        match self {
            TriggerScope::Transaction(install, scope) => match scope {
                super::Scope::Stateful => install.root.join(path),
                super::Scope::Ephemeral { blit_root } => blit_root.join(path),
            },
            TriggerScope::System(install, scope) => match scope {
                super::Scope::Stateful => install.root.join(path),
                super::Scope::Ephemeral { blit_root } => blit_root.join(path),
            },
        }
    }

    /// Join guest paths, inside the staging filesystem. Ensure no sandbox break for ephemeral
    fn guest_path(&self, path: impl AsRef<Path>) -> PathBuf {
        match self {
            TriggerScope::Transaction(install, scope) => match scope {
                super::Scope::Stateful => install.staging_path(path),
                super::Scope::Ephemeral { blit_root } => blit_root.join(path),
            },
            TriggerScope::System(install, scope) => match scope {
                super::Scope::Stateful => install.root.join(path),
                super::Scope::Ephemeral { blit_root } => blit_root.join(path),
            },
        }
    }
}

#[derive(Debug)]
pub(super) struct TriggerRunner<'a> {
    scope: TriggerScope<'a>,
    trigger: TriggerCommand,
}

/// Construct an iterator of executable triggers for the given
/// scope, which can be used with nice progress bars.
pub(super) fn triggers<'a>(
    scope: TriggerScope<'a>,
    fstree: &vfs::tree::Tree<PendingFile>,
) -> Result<Vec<TriggerRunner<'a>>, Error> {
    let trigger_root = Path::new("usr").join("share").join("moss").join("triggers");

    // Load appropriate triggers from their locations and convert back to a vec of Trigger
    let triggers = match scope {
        TriggerScope::Transaction(install, client_scope) => {
            config::Manager::custom(scope.root_dir().join(trigger_root))
                .load::<TransactionTrigger>()
                .into_iter()
                .map(|t| t.0)
                .collect_vec()
        }
        TriggerScope::System(install, client_scope) => config::Manager::custom(scope.root_dir().join(trigger_root))
            .load::<SystemTrigger>()
            .into_iter()
            .map(|t| t.0)
            .collect_vec(),
    };

    // Load trigger collection, process all the paths, convert to scoped TriggerRunner vec
    let mut collection = triggers::Collection::new(triggers.iter())?;
    collection.process_paths(fstree.iter().map(|m| m.to_string()));
    let computed_commands = collection
        .bake()?
        .into_iter()
        .map(|trigger| TriggerRunner { scope, trigger })
        .collect_vec();
    Ok(computed_commands)
}

impl<'a> TriggerRunner<'a> {
    pub fn execute(&self) -> Result<(), Error> {
        match self.scope {
            TriggerScope::Transaction(install, client_scope) => {
                // TODO: Add caching support via /var/
                let isolation = Container::new(install.isolation_dir())
                    .networking(false)
                    .override_accounts(false)
                    .bind_ro(self.scope.host_path("etc"), "/etc")
                    .bind_rw(self.scope.guest_path("usr"), "/usr")
                    .work_dir("/");

                Ok(isolation.run(|| execute_trigger_directly(&self.trigger))?)
            }
            TriggerScope::System(install, client_scope) => {
                // OK, if the root == `/` then we can run directly, otherwise we need to containerise with RW.
                if install.root.to_string_lossy() == "/" {
                    Ok(execute_trigger_directly(&self.trigger)?)
                } else {
                    let isolation = Container::new(install.isolation_dir())
                        .networking(false)
                        .override_accounts(false)
                        .bind_rw(self.scope.host_path("etc"), "/etc")
                        .bind_rw(self.scope.guest_path("usr"), "/usr")
                        .work_dir("/");

                    Ok(isolation.run(|| execute_trigger_directly(&self.trigger))?)
                }
            }
        }
    }
}

/// Internal executor for triggers.
fn execute_trigger_directly(trigger: &TriggerCommand) -> Result<(), Error> {
    match &trigger.handler {
        Handler::Run { run, args } => {
            let cmd = process::Command::new(run).args(args).current_dir("/").output()?;

            if let Some(code) = cmd.status.code() {
                if code != 0 {
                    eprintln!("Trigger exited with non-zero status code: {run} {args:?}");
                    eprintln!("   Stdout: {}", String::from_utf8(cmd.stdout).unwrap());
                    eprintln!("   Stderr: {}", String::from_utf8(cmd.stderr).unwrap());
                }
            } else {
                eprintln!("Failed to execute trigger: {run} {args:?}");
            }
        }
        Handler::Delete { delete } => todo!(),
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("container")]
    Container(#[from] container::Error),

    #[error("triggers")]
    Triggers(#[from] triggers::Error),

    #[error("io")]
    IO(#[from] std::io::Error),
}
