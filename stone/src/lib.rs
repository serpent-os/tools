// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

pub mod header;
mod reader;

pub use self::header::Header;
pub use self::reader::{from_bytes, from_reader, ReadError};

// TODO: Add typed payload
pub struct Stone {
    pub header: Header,
    pub payload: Vec<u8>,
}
