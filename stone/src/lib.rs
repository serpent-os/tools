// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{Read, Result, Write};

pub mod header;
pub mod payload;
pub mod read;
pub mod write;

pub use self::header::Header;
pub use self::read::{read, read_bytes, Reader};
pub use self::write::Writer;

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

    fn read_string(&mut self, length: u64) -> Result<String> {
        let mut string = String::with_capacity(length as usize);
        self.take(length).read_to_string(&mut string)?;
        Ok(string)
    }
}

impl<T: Read> ReadExt for T {}

pub trait WriteExt: Write {
    fn write_u8(&mut self, item: u8) -> Result<()> {
        self.write_array([item])
    }

    fn write_u16(&mut self, item: u16) -> Result<()> {
        self.write_array(item.to_be_bytes())
    }

    fn write_u32(&mut self, item: u32) -> Result<()> {
        self.write_array(item.to_be_bytes())
    }

    fn write_u64(&mut self, item: u64) -> Result<()> {
        self.write_array(item.to_be_bytes())
    }

    fn write_u128(&mut self, item: u128) -> Result<()> {
        self.write_array(item.to_be_bytes())
    }

    fn write_array<const N: usize>(&mut self, bytes: [u8; N]) -> Result<()> {
        self.write_all(&bytes)?;
        Ok(())
    }
}

impl<T: Write> WriteExt for T {}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn roundtrip() {
        let in_stone = include_bytes!("../../test/bash-completion-2.11-1-1-x86_64.stone");

        let mut reader = read_bytes(in_stone).unwrap();

        let payloads = reader
            .payloads()
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let meta = payloads.iter().find_map(read::Payload::meta).unwrap();
        let layouts = payloads.iter().find_map(read::Payload::layout).unwrap();
        let indices = payloads.iter().find_map(read::Payload::index).unwrap();
        let content = payloads.iter().find_map(read::Payload::content).unwrap();

        let mut content_buffer = vec![];

        reader.unpack_content(content, &mut content_buffer).unwrap();

        let mut out_stone = vec![];
        let mut writer = Writer::new(&mut out_stone, header::v1::FileType::Binary).unwrap();
        writer.add_meta_payload(meta).unwrap();
        writer.add_layout_payload(layouts).unwrap();

        let mut temp_content_buffer: Vec<u8> = vec![];
        let mut writer = writer
            .with_content(
                Cursor::new(&mut temp_content_buffer),
                Some(content_buffer.len() as u64),
            )
            .unwrap();

        for index in indices {
            let mut bytes = &content_buffer[index.start as usize..index.end as usize];

            writer.add_file(&mut bytes).unwrap();
        }

        writer.finalize().unwrap();

        let mut rt_reader = read_bytes(&out_stone).unwrap();
        assert_eq!(rt_reader.header, reader.header);

        let rt_payloads = rt_reader
            .payloads()
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let rt_meta = rt_payloads.iter().find_map(read::Payload::meta).unwrap();
        let rt_layouts = rt_payloads.iter().find_map(read::Payload::layout).unwrap();
        let rt_indices = rt_payloads.iter().find_map(read::Payload::index).unwrap();
        let rt_content = rt_payloads.iter().find_map(read::Payload::content).unwrap();

        assert_eq!(rt_meta.len(), meta.len());
        assert_eq!(rt_layouts.len(), layouts.len());
        assert_eq!(rt_indices.len(), indices.len());
        assert_eq!(rt_content.plain_size, content.plain_size);

        assert!(meta.iter().zip(rt_meta).all(|(a, b)| a == b));
        assert!(layouts.iter().zip(rt_layouts).all(|(a, b)| a == b));
        assert!(indices.iter().zip(rt_indices).all(|(a, b)| a == b));

        let mut rt_content_buffer = vec![];

        rt_reader
            .unpack_content(rt_content, &mut rt_content_buffer)
            .unwrap();

        assert_eq!(rt_content_buffer, content_buffer);

        println!(
            "Boulder-D stone size => {}, stone-rs stone size => {}",
            in_stone.len(),
            out_stone.len()
        );
    }
}
