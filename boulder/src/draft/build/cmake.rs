// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::path::Path;

use crate::draft::build::{Error, Phases, State};

pub fn phases() -> Phases {
    Phases {
        environment: None,
        setup: Some("%cmake"),
        build: Some("%cmake_build"),
        install: Some("%cmake_install"),
        check: None,
    }
}

pub fn process(state: &mut State, path: &Path) -> Result<(), Error> {
    // Depth too great
    if path.iter().count() > 2 {
        return Ok(());
    }

    if path.ends_with("CMakeLists.txt") {
        state.increment_confidence(20);
    }

    Ok(())
}
