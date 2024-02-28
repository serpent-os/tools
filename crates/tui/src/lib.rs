// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{Read, Write};

pub use self::reexport::*;

pub mod pretty;

const DEFAULT_TERM_SIZE: (u16, u16) = (80, 24);

/// Simple terminal constraints wrapping
pub struct TermSize {
    pub width: usize,
    pub height: usize,
}

/// Generate a sane-fallback TermSize
pub fn term_size() -> TermSize {
    let size = crossterm::terminal::size().unwrap_or(DEFAULT_TERM_SIZE);
    let mapped = if size.0 < 1 || size.1 < 1 {
        DEFAULT_TERM_SIZE
    } else {
        size
    };
    TermSize {
        width: mapped.0 as usize,
        height: mapped.1 as usize,
    }
}

/// Wraps a [`Write`] and updates the provided [`ProgressBar`] with progress
/// of total bytes written
pub struct ProgressWriter<W> {
    pub writer: W,
    pub total: u64,
    pub written: u64,
    pub progress: ProgressBar,
}

impl<W> ProgressWriter<W> {
    pub fn new(writer: W, total: u64, progress: ProgressBar) -> Self {
        Self {
            writer,
            total,
            written: 0,
            progress,
        }
    }
}

impl<W: Write> Write for ProgressWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let bytes = self.writer.write(buf)?;

        self.written += bytes as u64;
        self.progress.set_position(
            (self.written as f64 / self.total as f64 * self.progress.length().unwrap_or_default() as f64) as u64,
        );

        Ok(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// Wraps a [`Read`] and updates the provided [`ProgressBar`] with progress
/// of total bytes read
pub struct ProgressReader<R> {
    pub reader: R,
    pub total: u64,
    pub read: u64,
    pub progress: ProgressBar,
}

impl<R> ProgressReader<R> {
    pub fn new(reader: R, total: u64, progress: ProgressBar) -> Self {
        Self {
            reader,
            total,
            read: 0,
            progress,
        }
    }
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.reader.read(buf)?;

        self.read += read as u64;
        self.progress.set_position(
            (self.read as f64 / self.total as f64 * self.progress.length().unwrap_or_default() as f64) as u64,
        );

        Ok(read)
    }
}

/// Provide a standard approach to ratatui based TUI in moss
mod reexport {
    pub use crossterm::style::Stylize;
    pub use dialoguer;
    pub use indicatif::*;
}
