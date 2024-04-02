// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use crate::draft::build::{Error, Phases, State};
use crate::draft::File;

pub fn environment() -> &'static str {
    "export HOME=$(pwd)\n    export CARGO_HTTP_CAINFO=/usr/share/defaults/etc/ssl/certs/ca-certificates.crt"
}

pub fn phases() -> Phases {
    Phases {
        setup: Some("%cargo_fetch"),
        build: Some("%cargo_build"),
        install: Some("%cargo_install"),
        check: Some("%cargo_install"),
    }
}

pub fn process(state: &mut State, file: &File) -> Result<(), Error> {
    if file.path.ends_with("Cargo.toml") {
        state.increment_confidence(100);
    }

    Ok(())
}
