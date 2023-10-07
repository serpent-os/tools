// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

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

/// Provide a standard approach to ratatui based TUI in moss
mod reexport {
    pub use crossterm::style::Stylize;
    pub use indicatif::*;
}
