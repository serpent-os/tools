// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;
use thiserror::Error;

// TODO

#[derive(Debug, Error)]
pub enum WriteError {
    #[error(transparent)]
    Io(#[from] io::Error),
}
