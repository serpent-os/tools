// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::Command;
use moss::environment;

/// Construct the Version command
pub fn command() -> Command {
    Command::new("version").about("Display version and exit")
}

/// Print program version
pub fn print() {
    let hash = environment::GIT_HASH
        .map(|hash| format!(" ({hash})"))
        .unwrap_or_default();

    println!("moss {}{hash}", environment::VERSION);
}
