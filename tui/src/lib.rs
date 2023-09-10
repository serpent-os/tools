pub use self::program::Program;
pub use self::reexport::*;
pub use self::runtime::{run, Handle};

mod program;
mod runtime;
pub mod widget;

mod reexport {
    use std::io::Stdout;

    use ratatui::prelude::CrosstermBackend;
    pub use ratatui::prelude::{Constraint, Direction, Layout, Rect};

    pub type Frame<'a> = ratatui::Frame<'a, CrosstermBackend<Stdout>>;
}
