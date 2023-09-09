use std::io::{stdout, Result, Stdout};

use ratatui::{
    prelude::{CrosstermBackend, Rect},
    text::Line,
    widgets::{Paragraph, Widget},
    TerminalOptions, Viewport,
};

pub struct Terminal(ratatui::Terminal<CrosstermBackend<Stdout>>);

impl Terminal {
    pub fn new() -> Result<Self> {
        let stdout = stdout();
        let backend = CrosstermBackend::new(stdout);

        let mut terminal = ratatui::Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(10),
            },
        )?;
        terminal.autoresize()?;

        Ok(Self(terminal))
    }

    pub fn print(&mut self, content: String) -> Result<()> {
        let lines = content.lines().collect::<Vec<_>>();
        let num_lines = lines.len();
        let paragraph = Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>());

        self.0
            .insert_before(num_lines as u16, |buf| paragraph.render(buf.area, buf))?;

        Ok(())
    }

    pub fn resize(&mut self, lines: u16) -> Result<()> {
        let size = self.0.size()?;
        self.0.resize(Rect {
            height: lines,
            ..size
        })?;
        Ok(())
    }

    pub fn finish(mut self) {
        self.0.clear();
    }
}
