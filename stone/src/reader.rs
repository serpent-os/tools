// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use core::slice;
use std::{
    io::{BufRead, Result},
    mem::{size_of, zeroed},
};

use crate::header::AgnosticHeader;

#[cfg(test)]
mod reader_tests {
    use super::new;

    /// Header for bash completion stone archive
    const BASH_TEST_STONE: [u8; 32] = [
        0x0, 0x6d, 0x6f, 0x73, 0x0, 0x4, 0x0, 0x0, 0x1, 0x0, 0x0, 0x2, 0x0, 0x0, 0x3, 0x0, 0x0,
        0x4, 0x0, 0x0, 0x5, 0x0, 0x0, 0x6, 0x0, 0x0, 0x7, 0x1, 0x0, 0x0, 0x0, 0x1,
    ];

    /// Legacy manifest archive
    const TEST_MANIFEST: [u8; 32] = [
        0x0, 0x6d, 0x6f, 0x73, 0x0, 0x1, 0x0, 0x0, 0x1, 0x0, 0x0, 0x2, 0x0, 0x0, 0x3, 0x0, 0x0,
        0x4, 0x0, 0x0, 0x5, 0x0, 0x0, 0x6, 0x0, 0x0, 0x7, 0x4, 0x0, 0x0, 0x0, 0x1,
    ];

    #[test]
    fn test_versioning() {
        // Construct a reader from a byte sequence
        let mut reader: Box<dyn std::io::BufRead> = Box::new(&BASH_TEST_STONE[..]);
        let _ = new(&mut reader).unwrap();
    }
}

///
/// Create a new reader for the given byte sequence
///
pub fn new(bytes: &mut dyn BufRead) -> Result<()> {
    let mut hdr: AgnosticHeader = unsafe { zeroed::<AgnosticHeader>() };

    // Grab the AgnosticHeader from the 32-byte header
    let slice = unsafe {
        slice::from_raw_parts_mut(&mut hdr as *mut _ as *mut u8, size_of::<AgnosticHeader>())
    };
    bytes.read_exact(slice)?;

    // Testing: Ensure we're reading V1.
    #[cfg(test)]
    {
        use crate::header::{self, Header};

        let decoded = Header::decode(hdr).expect("valid header");
        assert_eq!(decoded.version(), header::Version::V1);
    }

    Ok(())
}
