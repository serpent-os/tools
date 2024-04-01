// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::path::Path;

use crate::draft::build::{Error, Phases, State};

pub fn phases() -> Phases {
    Phases {
        environment: Some(
            "export HOME=$(pwd)\n    export CARGO_HTTP_CAINFO=/usr/share/defaults/etc/ssl/certs/ca-certificates.crt",
        ),
        setup: Some("%cargo_fetch"),
        build: Some("%cargo_build"),
        install: Some("%cargo_install"),
        check: Some("%cargo_install"),
    }
}

pub fn process(state: &mut State, path: &Path) -> Result<(), Error> {
    if path.ends_with("Cargo.toml") {
        state.increment_confidence(100);
    }

    Ok(())
}
