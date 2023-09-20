// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

pub use self::program::Program;
pub use self::reexport::*;
pub use self::runtime::{run, Handle};

mod program;
mod runtime;
pub mod widget;

/// Provide a standard approach to ratatui based TUI in moss
mod reexport {
    use std::io::Stdout;

    pub use crossterm::style::Stylize;
    use ratatui::prelude::CrosstermBackend;
    pub use ratatui::prelude::{Constraint, Direction, Layout, Rect};

    pub type Frame<'a> = ratatui::Frame<'a, CrosstermBackend<Stdout>>;
}
