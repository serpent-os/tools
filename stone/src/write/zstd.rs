// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{Result, Write};

use zstd::stream::raw::Operation;
use zstd::zstd_safe::CParameter;

type RawEncoder = zstd::stream::raw::Encoder<'static>;
type ZstdWriter<'a, W> = zstd::stream::zio::Writer<W, &'a mut Encoder>;

/// Transparent encapsulation of zstd compression with the purpose
/// of encoding moss (.stone) payloads to a stream
pub struct Writer<'a, W: Write> {
    writer: ZstdWriter<'a, W>,
    pub plain_bytes: usize,
}

impl<'a, W: Write> Writer<'a, W> {
    /// Construct a new Writerfor the given writer
    pub fn new(writer: W, encoder: &'a mut Encoder) -> Self {
        Self {
            writer: ZstdWriter::new(writer, encoder),
            plain_bytes: 0,
        }
    }

    /// Finish all encoding and flush underlying writer
    pub fn finish(self) -> Result<()> {
        let (mut writer, encoder) = self.writer.into_inner();
        let footer = encoder.finish()?;
        writer.write_all(&footer)?;
        writer.flush()
    }
}

impl<'a, W: Write> Write for Writer<'a, W> {
    /// Handle transparent encoding, record offsets
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.plain_bytes += buf.len();

        self.writer.write(buf)
    }

    /// Flush the stream
    fn flush(&mut self) -> Result<()> {
        self.writer.flush()
    }
}

pub struct Encoder(RawEncoder);

impl Encoder {
    /// Concrete zstd encoder
    pub fn new() -> Result<Self> {
        let mut encoder = RawEncoder::new(18)?;
        encoder.set_parameter(CParameter::WindowLog(31))?;
        Ok(Self(encoder))
    }

    /// Let zstd know of the final uncompressed size, to optimise compression
    pub fn set_pledged_size(&mut self, pledged_size: Option<u64>) -> Result<()> {
        self.0.set_pledged_src_size(pledged_size)
    }

    /// Flush everything
    pub fn finish(&mut self) -> Result<Vec<u8>> {
        let mut footer = vec![];
        let mut writer = ZstdWriter::new(&mut footer, self);
        writer.finish()?;
        self.0.reinit()?;
        self.0.set_pledged_src_size(None)?;
        Ok(footer)
    }
}

impl<'a> Operation for &'a mut Encoder {
    fn run<C: zstd::zstd_safe::WriteBuf + ?Sized>(
        &mut self,
        input: &mut zstd::zstd_safe::InBuffer<'_>,
        output: &mut zstd::zstd_safe::OutBuffer<'_, C>,
    ) -> std::io::Result<usize> {
        self.0.run(input, output)
    }

    fn run_on_buffers(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> std::io::Result<zstd::stream::raw::Status> {
        self.0.run_on_buffers(input, output)
    }

    fn flush<C: zstd::zstd_safe::WriteBuf + ?Sized>(
        &mut self,
        output: &mut zstd::zstd_safe::OutBuffer<'_, C>,
    ) -> std::io::Result<usize> {
        self.0.flush(output)
    }

    fn reinit(&mut self) -> std::io::Result<()> {
        self.0.reinit()
    }

    fn finish<C: zstd::zstd_safe::WriteBuf + ?Sized>(
        &mut self,
        output: &mut zstd::zstd_safe::OutBuffer<'_, C>,
        finished_frame: bool,
    ) -> std::io::Result<usize> {
        self.0.finish(output, finished_frame)
    }
}
