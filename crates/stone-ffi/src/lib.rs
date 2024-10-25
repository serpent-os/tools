// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::missing_safety_doc)]
#![allow(non_camel_case_types)]

use std::{
    fs::File,
    io::{Cursor, Write},
    os::fd::FromRawFd,
    ptr::NonNull,
    slice,
};

use libc::{c_int, size_t};
use stone::{
    StoneDecodedPayload, StoneHeader, StoneHeaderV1, StoneHeaderV1FileType, StoneHeaderVersion,
    StonePayloadCompression, StonePayloadHeader, StonePayloadKind, StonePayloadLayoutFileType, StoneReadError,
};

pub use self::payload::{StonePayload, StonePayloadLayoutRecord};

mod payload;

#[derive(Debug)]
#[repr(C)]
pub struct StoneString {
    pub buf: *const u8,
    pub size: size_t,
}

impl StoneString {
    pub fn new(s: &str) -> Self {
        Self {
            buf: s.as_ptr(),
            size: s.len(),
        }
    }
}

pub enum StoneReader<'a> {
    File(stone::StoneReader<File>),
    Buffer(stone::StoneReader<Cursor<&'a [u8]>>),
}

impl<'a> StoneReader<'a> {
    pub fn header(&self) -> &StoneHeader {
        match self {
            StoneReader::File(reader) => &reader.header,
            StoneReader::Buffer(reader) => &reader.header,
        }
    }

    pub fn next_payload(&mut self) -> Result<Option<StoneDecodedPayload>, StoneReadError> {
        match self {
            StoneReader::File(reader) => reader.next_payload(),
            StoneReader::Buffer(reader) => reader.next_payload(),
        }
    }

    pub fn unpack_content<W>(
        &mut self,
        content: &stone::StonePayload<stone::StonePayloadContent>,
        writer: &mut W,
    ) -> Result<(), StoneReadError>
    where
        W: Write,
    {
        match self {
            StoneReader::File(reader) => reader.unpack_content(content, writer),
            StoneReader::Buffer(reader) => reader.unpack_content(content, writer),
        }
    }
}

fn fallible(f: impl FnOnce() -> Result<(), Box<dyn std::error::Error>>) -> c_int {
    if f().is_err() {
        -1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn stone_reader_read_file(
    file: c_int,
    reader_ptr: *mut *mut StoneReader,
    version: *mut StoneHeaderVersion,
) -> c_int {
    fallible(|| {
        // TODO: Errors
        let reader_ptr = NonNull::new(reader_ptr).ok_or("")?;
        let mut version = NonNull::new(version).ok_or("")?;

        let reader = stone::read(File::from_raw_fd(file)).map(StoneReader::File)?;

        *version.as_mut() = reader.header().version();
        *reader_ptr.as_ptr() = Box::into_raw(Box::new(reader));

        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn stone_reader_read_buf(
    buf: *const u8,
    len: usize,
    reader_ptr: *mut *mut StoneReader,
    version: *mut StoneHeaderVersion,
) -> c_int {
    fallible(|| {
        let buf = NonNull::new(buf as *mut _).ok_or("")?;
        let reader_ptr = NonNull::new(reader_ptr).ok_or("")?;
        let mut version = NonNull::new(version).ok_or("")?;

        let reader = stone::read_bytes(slice::from_raw_parts(buf.as_ptr(), len)).map(StoneReader::Buffer)?;

        *version.as_mut() = reader.header().version();
        *reader_ptr.as_ptr() = Box::into_raw(Box::new(reader));

        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn stone_reader_header_v1(reader: *const StoneReader, header: *mut StoneHeaderV1) -> c_int {
    fallible(|| {
        let reader = NonNull::new(reader as *mut StoneReader).ok_or("")?;
        let mut header = NonNull::new(header).ok_or("")?;

        match reader.as_ref().header() {
            StoneHeader::V1(v1) => *header.as_mut() = *v1,
        }

        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn stone_reader_next_payload(
    reader: *mut StoneReader,
    payload_ptr: *mut *mut StonePayload,
) -> c_int {
    fallible(|| {
        let mut reader = NonNull::new(reader).ok_or("")?;
        let payload_ptr = NonNull::new(payload_ptr).ok_or("")?;

        if let Some(payload) = reader.as_mut().next_payload()? {
            *payload_ptr.as_ptr() = Box::into_raw(Box::new(payload.into()));
        } else {
            Err("no more payloads")?;
        }

        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn stone_reader_unpack_content_payload(
    reader: *mut StoneReader,
    payload: *const StonePayload,
    data: *mut u8,
) -> c_int {
    fallible(|| {
        let mut reader = NonNull::new(reader).ok_or("")?;
        let payload = NonNull::new(payload as *mut StonePayload).ok_or("")?;
        let data = NonNull::new(data).ok_or("")?;

        let mut cursor = Cursor::new(slice::from_raw_parts_mut(
            data.as_ptr(),
            payload.as_ref().header().stored_size as usize,
        ));

        if let StoneDecodedPayload::Content(content) = &payload.as_ref().decoded {
            reader.as_mut().unpack_content(content, &mut cursor)?;
        } else {
            Err("incorrect payload kind")?;
        }

        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn stone_reader_destroy(reader: *mut StoneReader) {
    let Some(reader) = NonNull::new(reader) else {
        return;
    };

    drop(Box::from_raw(reader.as_ptr()));
}

#[no_mangle]
pub unsafe extern "C" fn stone_payload_header(payload: *const StonePayload, header: *mut StonePayloadHeader) -> c_int {
    fallible(|| {
        let payload = NonNull::new(payload as *mut StonePayload).ok_or("")?;
        let mut header = NonNull::new(header).ok_or("")?;

        *header.as_mut() = *payload.as_ref().header();

        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn stone_payload_next_layout_record(
    payload: *mut StonePayload,
    record: *mut StonePayloadLayoutRecord,
) -> c_int {
    fallible(|| {
        let mut payload = NonNull::new(payload).ok_or("")?;
        let mut record = NonNull::new(record).ok_or("")?;

        *record.as_mut() = payload.as_mut().next_layout_record().ok_or("")?;

        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn stone_payload_destroy(payload: *mut StonePayload) {
    let Some(payload) = NonNull::new(payload) else {
        return;
    };

    drop(Box::from_raw(payload.as_ptr()));
}

#[no_mangle]
pub unsafe extern "C" fn stone_format_header_v1_file_type(file_type: StoneHeaderV1FileType, buf: *mut u8) {
    fill_c_string(buf, file_type);
}

#[no_mangle]
pub unsafe extern "C" fn stone_format_payload_compression(compression: StonePayloadCompression, buf: *mut u8) {
    fill_c_string(buf, compression);
}

#[no_mangle]
pub unsafe extern "C" fn stone_format_payload_kind(kind: StonePayloadKind, buf: *mut u8) {
    fill_c_string(buf, kind);
}

#[no_mangle]
pub unsafe extern "C" fn stone_format_payload_layout_file_type(file_type: StonePayloadLayoutFileType, buf: *mut u8) {
    fill_c_string(buf, file_type);
}

unsafe fn fill_c_string(buf: *mut u8, content: impl ToString) {
    let Some(buf) = NonNull::new(buf) else {
        return;
    };

    let content = content.to_string();
    let buf = slice::from_raw_parts_mut(buf.as_ptr(), content.len() + 1);

    buf[0..content.len()].copy_from_slice(content.as_bytes());
    buf[content.len()] = b'\0';
}
