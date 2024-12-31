// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use crate::draft::build::{Error, Phases, State};
use crate::draft::DrafterFile;

pub fn phases() -> Phases {
    Phases {
        setup: Some("%cmake"),
        build: Some("%cmake_build"),
        install: Some("%cmake_install"),
        check: None,
    }
}

pub fn process(state: &mut State, file: &DrafterFile) -> Result<(), Error> {
    // Depth too great
    if file.depth() > 0 {
        return Ok(());
    }

    if file.file_name() == "CMakeLists.txt" {
        state.increment_confidence(20);
    }

    Ok(())
}
