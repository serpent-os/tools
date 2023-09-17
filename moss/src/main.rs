// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

mod cli;

/// Main entry point
#[tokio::main]
async fn main() -> Result<(), cli::Error> {
    cli::process().await
}
