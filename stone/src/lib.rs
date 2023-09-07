// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{Read, Result};

pub mod header;
pub mod payload;
pub mod read;
mod write;

pub use self::header::Header;
pub use self::read::{read, read_bytes};

pub trait ReadExt: Read {
    fn read_u8(&mut self) -> Result<u8> {
        let bytes = self.read_array::<1>()?;
        Ok(bytes[0])
    }

    fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_array()?;
        Ok(u16::from_be_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_array()?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.read_array()?;
        Ok(u64::from_be_bytes(bytes))
    }

    fn read_u128(&mut self) -> Result<u128> {
        let bytes = self.read_array()?;
        Ok(u128::from_be_bytes(bytes))
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut bytes = [0u8; N];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn read_vec(&mut self, length: usize) -> Result<Vec<u8>> {
        let mut bytes = vec![0u8; length];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

impl<T: Read> ReadExt for T {}
