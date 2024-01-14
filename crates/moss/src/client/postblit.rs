// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Operations that happen post-blit (primarily, triggers within container)

use container::Container;
use thiserror::Error;

use crate::{environment, Client, Installation};

use super::create_root_links;

/// Begin the postblit processing
pub(crate) async fn handle_postblits(client: &Client, install: &Installation) -> Result<(), Error> {
    create_root_links(&install.isolation_dir()).await?;
    let isolation = Container::new(install.isolation_dir())
        .networking(false)
        .bind_rw(install.root.join("etc"), "/etc")
        .bind_rw(install.staging_path("usr"), "/usr")
        .hostname(environment::NAME)
        .work_dir("/");

    Ok(isolation.run(trigger_runner)?)
}

/// Post-blit trigger runner
fn trigger_runner() -> Result<(), Error> {
    eprintln!("In the container.");
    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("container")]
    Container(#[from] container::Error),

    #[error("io")]
    IO(#[from] std::io::Error),
}
