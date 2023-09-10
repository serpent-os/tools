// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fs::{remove_file, File},
    io::Write,
    path::PathBuf,
};

use clap::{arg, ArgMatches, Command};
use thiserror::{self, Error};
use tui::{widget::progress, Constraint, Direction, Frame, Handle, Layout, Rect};

pub fn command() -> Command {
    Command::new("extract")
        .about("Extract a `.stone` content to disk")
        .long_about("For all valid content-bearing archives, extract to disk")
        .arg(arg!(<PATH> ... "files to inspect").value_parser(clap::value_parser!(PathBuf)))
}

/// Handle the `extract` command
pub fn handle(args: &ArgMatches) -> Result<(), Error> {
    let paths = args
        .get_many::<PathBuf>("PATH")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    tui::run(Program::default(), move |mut handle| {
        // Begin unpack
        for path in paths {
            handle.print(format!("Extract: {:?}", path));

            let rdr = File::open(path).map_err(Error::IO)?;
            let mut reader = stone::read(rdr).map_err(Error::Format)?;

            if let Some(content) = reader.content {
                let size = content.plain_size;

                let mut writer =
                    ProgressWriter::new(File::create(".stoneContent")?, size, handle.clone());
                reader.unpack_content(reader.content.unwrap(), &mut writer)?;

                remove_file(".stoneContent")?;
            }
        }

        Ok(()) as Result<(), Error>
    })??;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Read failure")]
    IO(#[from] std::io::Error),

    #[error("Format failure")]
    Format(#[from] stone::read::Error),
}

struct ProgressWriter<W> {
    writer: W,
    total: u64,
    written: u64,
    handle: Handle<Message>,
}

impl<W> ProgressWriter<W> {
    pub fn new(writer: W, total: u64, handle: Handle<Message>) -> Self {
        Self {
            writer,
            total,
            written: 0,
            handle,
        }
    }
}

impl<W: Write> Write for ProgressWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let bytes = self.writer.write(buf)?;

        self.written += bytes as u64;
        self.handle
            .update(Message::Progress(self.written as f64 / self.total as f64));

        Ok(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Default)]
struct Program {
    progress: f32,
}

enum Message {
    Progress(f64),
}

impl tui::Program for Program {
    const LINES: u16 = 3;

    type Message = Message;

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::Progress(progess) => self.progress = progess as f32,
        }
    }

    fn draw(&self, frame: &mut Frame) {
        const MAX_WIDTH: u16 = 20;

        let layout = Layout::new()
            .direction(Direction::Vertical)
            .vertical_margin(1)
            .constraints([Constraint::Length(1)])
            .split(frame.size());

        let width = u16::min(MAX_WIDTH, layout[0].width);

        frame.render_widget(
            progress(self.progress, progress::Fill::UpAcross),
            Rect { width, ..layout[0] },
        );
    }
}
