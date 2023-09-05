// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::error::Error;

use moss::cli;

/// Main entry point
fn main() -> Result<(), Box<dyn Error>> {
    cli::process()?;

    Ok(())
}
