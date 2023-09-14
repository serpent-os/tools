// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io::{stdout, Result},
    time::Duration,
};

use async_signal::{Signal, Signals};
use futures::{stream, FutureExt, StreamExt};
use ratatui::{
    prelude::CrosstermBackend,
    text::Line,
    widgets::{Paragraph, Widget},
    TerminalOptions, Viewport,
};
use smol::{
    channel::{self, Sender},
    Timer,
};

use crate::Program;

pub fn run<P: Program, T: Send>(
    mut program: P,
    f: impl FnOnce(Handle<P::Message>) -> T + Send + Sync + 'static,
) -> Result<T>
where
    P::Message: Send + 'static,
    T: 'static,
{
    smol::block_on(async move {
        // Ctrl-c capture
        let ctrl_c = Signals::new([Signal::Int])?;

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
        let (sender, receiver) = channel::unbounded();

        // We can receive user event or finished status
        enum Input<T> {
            Render,
            Finished(T),
            Term,
        }

        // Run task
        let mut run = smol::unblock(move || f(Handle { sender }))
            .boxed()
            .map(Input::Finished)
            .into_stream();
        // Ctrl c task
        let mut ctrl_c = ctrl_c.map(|_| Input::Term);
        // Rerender @ 60fps
        let mut interval =
            StreamExt::map(Timer::interval(Duration::from_millis(1000 / 60)), |_| {
                Input::Render
            });

        loop {
            // Get next input
            let input = stream::select(&mut run, stream::select(&mut interval, &mut ctrl_c))
                .next()
                .await
                .unwrap();

            match input {
                Input::Render => {
                    let mut print = vec![];

                    while let Ok(event) = receiver.try_recv() {
                        match event {
                            Event::Message(message) => program.update(message),
                            Event::Print(content) => print.push(content),
                        }
                    }

                    if !print.is_empty() {
                        let lines = print
                            .iter()
                            .flat_map(|content| content.lines())
                            .collect::<Vec<_>>();
                        let num_lines = lines.len();
                        let paragraph =
                            Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>());

                        terminal.insert_before(num_lines as u16, |buf| {
                            paragraph.render(buf.area, buf)
                        })?;
                    }

                    terminal.draw(|frame| program.draw(frame))?;
                }
                Input::Finished(t) => {
                    terminal.show_cursor()?;
                    terminal.clear()?;
                    return Ok(t);
                }
                Input::Term => {
                    terminal.show_cursor()?;
                    terminal.clear()?;
                    std::process::exit(0);
                }
            }
        }
    })
}

pub struct Handle<Message> {
    sender: Sender<Event<Message>>,
}

impl<Message> Clone for Handle<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<Message> Handle<Message> {
    pub fn print(&mut self, content: String) {
        let _ = self.sender.send_blocking(Event::Print(content));
    }

    pub fn update(&mut self, message: Message) {
        let _ = self.sender.send_blocking(Event::Message(message));
    }
}

pub enum Event<Message> {
    Message(Message),
    Print(String),
}
