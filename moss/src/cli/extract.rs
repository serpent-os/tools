// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fs::{create_dir_all, hard_link, remove_dir_all, remove_file, File},
    io::{copy, Read, Seek, SeekFrom, Write},
    os::unix::fs::symlink,
    path::PathBuf,
};

use clap::{arg, ArgMatches, Command};
use stone::{payload::layout, read::Payload};
use thiserror::{self, Error};
use tokio::task;
use tui::{widget::progress, Constraint, Direction, Frame, Handle, Layout};

pub fn command() -> Command {
    Command::new("extract")
        .about("Extract a `.stone` content to disk")
        .long_about("For all valid content-bearing archives, extract to disk")
        .arg(arg!(<PATH> ... "files to inspect").value_parser(clap::value_parser!(PathBuf)))
}

/// Handle the `extract` command
pub async fn handle(args: &ArgMatches) -> Result<(), Error> {
    let paths = args
        .get_many::<PathBuf>("PATH")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    tui::run(Program::default(), |handle| async move {
        task::spawn_blocking(move || extract(paths, handle))
            .await
            .expect("join handle")
    })
    .await??;

    Ok(())
}

fn extract(paths: Vec<PathBuf>, mut handle: tui::Handle<Message>) -> Result<(), Error> {
    // Begin unpack
    create_dir_all(".stoneStore")?;

    let content_store = PathBuf::from(".stoneStore");
    let extraction_root = PathBuf::from("extracted");

    for path in paths {
        handle.print(format!("Extract: {:?}", path));

        let rdr = File::open(path).map_err(Error::IO)?;
        let mut reader = stone::read(rdr).map_err(Error::Format)?;

        let payloads = reader.payloads()?.collect::<Result<Vec<_>, _>>()?;
        let content = payloads.iter().find_map(Payload::content);
        let layouts = payloads.iter().find_map(Payload::layout);

        if let Some(content) = content {
            let size = content.plain_size;

            let mut content_storage = File::options()
                .read(true)
                .write(true)
                .create(true)
                .open(".stoneContent")?;
            let mut writer = ProgressWriter::new(&mut content_storage, size, handle.clone());
            reader.unpack_content(content, &mut writer)?;

            // Rewind.
            content_storage.seek(SeekFrom::Start(0))?;

            // Extract all indices from the `.stoneContent` into hash-indexed unique files
            for idx in payloads.iter().filter_map(Payload::index).flatten() {
                let mut output = File::create(format!(".stoneStore/{:02x}", idx.digest))?;
                let mut split_file = (&mut content_storage).take(idx.end - idx.start);
                copy(&mut split_file, &mut output)?;
            }

            remove_file(".stoneContent")?;
        }

        if let Some(layouts) = layouts {
            for layout in layouts {
                match &layout.entry {
                    layout::Entry::Regular(id, target) => {
                        let store_path = content_store.join(format!("{:02x}", id));
                        let target_disk = extraction_root.join("usr").join(target);

                        // drop it into a valid dir
                        // TODO: Fix the permissions & mask
                        let directory_target = target_disk.parent().unwrap();
                        create_dir_all(directory_target)?;

                        // link from CA store
                        hard_link(store_path, target_disk)?;
                    }
                    layout::Entry::Symlink(source, target) => {
                        let target_disk = extraction_root.join("usr").join(target);
                        let directory_target = target_disk.parent().unwrap();

                        // ensure dumping ground exists
                        create_dir_all(directory_target)?;

                        // join the link path to the directory target for relative joinery
                        symlink(source, target_disk)?;
                    }
                    layout::Entry::Directory(target) => {
                        let target_disk = extraction_root.join("usr").join(target);
                        // TODO: Fix perms!
                        create_dir_all(target_disk)?;
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    // Clean up.
    remove_dir_all(content_store)?;

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
            Message::Progress(progress) => self.progress = progress as f32,
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let layout = Layout::new()
            .direction(Direction::Vertical)
            .vertical_margin(1)
            .constraints([Constraint::Length(1)])
            .split(frame.size());

        frame.render_widget(
            progress(self.progress, progress::Fill::UpAcross, 20),
            layout[0],
        );
    }
}
