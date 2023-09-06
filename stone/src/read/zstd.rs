use std::io::{BufReader, Read, Result};

use zstd::stream::read::Decoder;

pub struct Zstd<'a, R: Read> {
    decoder: Decoder<'a, BufReader<R>>,
}

impl<'a, R: Read> Zstd<'a, R> {
    pub fn new(reader: R) -> Result<Self> {
        Ok(Self {
            decoder: Decoder::new(reader)?,
        })
    }
}

impl<'a, R: Read> Read for Zstd<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.decoder.read(buf)
    }
}
