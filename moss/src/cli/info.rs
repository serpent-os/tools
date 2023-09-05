// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::Command;

pub fn command() -> Command {
    Command::new("info")
        .about("Query packages")
        .long_about("List detailed package information from all available sources")
}
