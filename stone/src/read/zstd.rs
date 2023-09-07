use std::io::{BufReader, Read, Result};

use zstd::stream::read::Decoder;

pub struct Zstd<'a, R: Read> {
    decoder: Decoder<'a, BufReader<R>>,
}

impl<'a, R: Read> Zstd<'a, R> {
    pub fn new(reader: R) -> Result<Self> {
        let mut decoder = Decoder::new(reader)?;
        decoder.window_log_max(31)?;

        Ok(Self { decoder })
    }
}

impl<'a, R: Read> Read for Zstd<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.decoder.read(buf)
    }
}
