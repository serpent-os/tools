// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io::{stdout, Result},
    time::Duration,
};

use futures::{stream, Future, FutureExt, StreamExt};
use ratatui::{
    prelude::CrosstermBackend,
    text::Line,
    widgets::{Paragraph, Widget},
    TerminalOptions, Viewport,
};
use tokio::{signal::ctrl_c, sync::mpsc, time};
use tokio_stream::wrappers::IntervalStream;

use crate::Program;

/// Run the TUI application within the async runtime and handle all
/// events automatically, including rendering and signals.
pub async fn run<P, F, T>(mut program: P, f: impl FnOnce(Handle<P::Message>) -> F) -> Result<T>
where
    P: Program,
    F: Future<Output = T> + Send,
{
    // Setup terminal
    let mut terminal = ratatui::Terminal::with_options(
        CrosstermBackend::new(stdout()),
        TerminalOptions {
            viewport: Viewport::Inline(P::LINES),
        },
    )?;

    // Draw initial view
    terminal.draw(|frame| {
        program.draw(frame);
    })?;

    // Setup channel
    let (sender, mut receiver) = mpsc::unbounded_channel();

    // We can receive render event or finished status
    enum Input<T> {
        Render,
        Finished(T),
        Term,
    }

    // Run task
    let mut run = f(Handle { sender })
        .map(Input::Finished)
        .into_stream()
        .boxed();
    // Ctrl c task
    let mut ctrl_c = ctrl_c().map(|_| Input::Term).into_stream().boxed();
    // Rerender @ 60fps
    let mut interval = IntervalStream::new(time::interval(Duration::from_millis(1000 / 60)))
        .map(|_| Input::Render);

    loop {
        // Get next input
        let input = stream::select(&mut run, stream::select(&mut ctrl_c, &mut interval))
            .next()
            .await
            .unwrap();

        let mut update = || {
            let mut lines = vec![];

            while let Ok(event) = receiver.try_recv() {
                match event {
                    Event::Message(message) => program.update(message),
                    Event::Print(line) => lines.push(line),
                }
            }

            if !lines.is_empty() {
                let num_lines = lines.len();
                let paragraph = Paragraph::new(lines);

                terminal.insert_before(num_lines as u16, |buf| paragraph.render(buf.area, buf))?;
            }

            terminal.draw(|frame| program.draw(frame))?;

            Ok(()) as Result<()>
        };

        match input {
            Input::Render => {
                update()?;
            }
            Input::Finished(ret) => {
                update()?;

                terminal.show_cursor()?;
                terminal.clear()?;

                return Ok(ret);
            }
            Input::Term => {
                terminal.show_cursor()?;
                terminal.clear()?;
                std::process::exit(0);
            }
        }
    }
}

pub struct Handle<Message> {
    sender: mpsc::UnboundedSender<Event<Message>>,
}

impl<Message> Clone for Handle<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<Message> Handle<Message> {
    pub fn print(&self, line: impl Into<Line<'static>>) {
        let _ = self.sender.send(Event::Print(line.into()));
    }

    pub fn update(&self, message: Message) {
        let _ = self.sender.send(Event::Message(message));
    }
}

pub enum Event<Message> {
    Message(Message),
    Print(Line<'static>),
}
