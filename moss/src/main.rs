// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

mod cli;

/// Main entry point
fn main() -> Result<(), cli::Error> {
    cli::process()
}
