// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEvent};

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

/// Read a single line of input (up to enter)
pub fn read_line() -> std::io::Result<String> {
    let mut s = String::new();
    while let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Enter => break,
            KeyCode::Char(c) => s.push(c),
            _ => {}
        }
    }
    Ok(s)
}

/// Prompt yes/no
pub fn ask_yes_no(question: &str) -> std::io::Result<bool> {
    print!(
        "{} {} {} / {} {} ",
        question,
        "[".dim(),
        "yes".bold(),
        "no".bold().red(),
        "]".dim()
    );
    io::stdout().flush()?;
    Ok(matches!(read_line()?.to_lowercase().as_str(), "y" | "yes"))
}

/// Provide a standard approach to ratatui based TUI in moss
mod reexport {
    pub use crossterm::style::Stylize;
    pub use indicatif::*;
}
