// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Operations that happen post-blit (primarily, triggers within container)
//! Note that we support transaction scope and system scope triggers, invoked
//! before `/usr` is activated and after, respectively.
//!
//! Note that currently we only load from `/usr/share/moss/triggers/{tx,sys.d}/*.yaml`
//! and do not yet support local triggers
use std::{path::Path, process};

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
    Transaction(&'a Installation),
    System(&'a Installation),
}

#[derive(Debug)]
pub(super) struct TriggerRunner<'a> {
    scope: TriggerScope<'a>,
    trigger: TriggerCommand,
}

/// Construct an iterator of executable triggers for the given
/// scope, which can be used with nice progress bars.
pub(super) async fn triggers<'a>(
    scope: TriggerScope<'a>,
    fstree: &vfs::tree::Tree<PendingFile>,
) -> Result<Vec<TriggerRunner<'a>>, Error> {
    let trigger_root = Path::new("usr").join("share").join("moss").join("triggers");

    // Load appropriate triggers from their locations and convert back to a vec of Trigger
    let triggers = match scope {
        TriggerScope::Transaction(install) => config::Manager::custom(install.staging_dir().join(trigger_root))
            .load::<TransactionTrigger>()
            .await
            .into_iter()
            .map(|t| t.0)
            .collect_vec(),
        TriggerScope::System(install) => config::Manager::custom(install.root.join(trigger_root))
            .load::<SystemTrigger>()
            .await
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
            TriggerScope::Transaction(install) => {
                // TODO: Add caching support via /var/
                let isolation = Container::new(install.isolation_dir())
                    .networking(false)
                    .override_accounts(false)
                    .bind_ro(install.root.join("etc"), "/etc")
                    .bind_rw(install.staging_path("usr"), "/usr")
                    .work_dir("/");

                Ok(isolation.run(|| execute_trigger_directly(&self.trigger))?)
            }
            TriggerScope::System(install) => {
                // OK, if the root == `/` then we can run directly, otherwise we need to containerise with RW.
                if install.root.to_string_lossy() == "/" {
                    Ok(execute_trigger_directly(&self.trigger)?)
                } else {
                    let isolation = Container::new(install.isolation_dir())
                        .networking(false)
                        .override_accounts(false)
                        .bind_rw(install.root.join("etc"), "/etc")
                        .bind_rw(install.root.join("usr"), "/usr")
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
