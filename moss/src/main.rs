// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use color_eyre::Result;

mod cli;

/// Main entry point
fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        // .display_env_section(false)
        .panic_section("Please report bugs to https://github.com/serpent-os/moss/issues/new")
        .display_env_section(false)
        .install()?;

    cli::process()?;

    Ok(())
}
