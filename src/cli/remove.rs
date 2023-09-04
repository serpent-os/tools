// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::Command;

pub fn remove_command() -> Command {
    Command::new("remove")
        .about("Remove packages")
        .long_about("Remove packages by name")
}
