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
use moss::package::{self, MissingMetaError};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use stone::{payload::layout, read::Payload};
use thiserror::{self, Error};
use tokio::task;
use tui::{ProgressBar, ProgressStyle};

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

    task::spawn_blocking(move || extract(paths))
        .await
        .expect("join handle")?;

    Ok(())
}

fn extract(paths: Vec<PathBuf>) -> Result<(), Error> {
    // Begin unpack
    create_dir_all(".stoneStore")?;

    let content_store = PathBuf::from(".stoneStore");

    for path in paths {
        println!("Extract: {:?}", path);

        let rdr = File::open(path).map_err(Error::IO)?;
        let mut reader = stone::read(rdr).map_err(Error::Format)?;

        let payloads = reader.payloads()?.collect::<Result<Vec<_>, _>>()?;
        let content = payloads.iter().find_map(Payload::content);
        let layouts = payloads.iter().find_map(Payload::layout);
        let meta = payloads
            .iter()
            .find_map(Payload::meta)
            .ok_or(Error::MissingMeta)?;

        let pkg = package::Meta::from_stone_payload(meta).map_err(Error::MalformedMeta)?;
        let extraction_root = PathBuf::from(pkg.id().to_string());

        // Cleanup old extraction root
        if extraction_root.exists() {
            remove_dir_all(&extraction_root)?;
        }

        let progress = ProgressBar::new(1000).with_style(
            ProgressStyle::with_template("|{bar:20.cyan/bue}| {percent}%")
                .unwrap()
                .progress_chars("■≡=- "),
        );

        if let Some(content) = content {
            let size = content.plain_size;

            let content_file = File::options()
                .read(true)
                .write(true)
                .create(true)
                .open(".stoneContent")?;

            let mut writer = ProgressWriter::new(&content_file, size, progress.clone());
            reader.unpack_content(content, &mut writer)?;

            // Extract all indices from the `.stoneContent` into hash-indexed unique files
            payloads
                .par_iter()
                .filter_map(Payload::index)
                .flatten()
                .map(|idx| {
                    // Split file reader over index range
                    let mut file = &content_file;
                    file.seek(SeekFrom::Start(idx.start))?;
                    let mut split_file = (&mut file).take(idx.end - idx.start);

                    let mut output = File::create(format!(".stoneStore/{:02x}", idx.digest))?;

                    copy(&mut split_file, &mut output)?;

                    Ok(())
                })
                .collect::<Result<Vec<_>, Error>>()?;

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

        progress.finish_and_clear();
    }

    // Clean up.
    remove_dir_all(content_store)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing metadata")]
    MissingMeta,

    #[error("malformed meta {0}")]
    MalformedMeta(#[from] MissingMetaError),

    #[error("Read failure")]
    IO(#[from] std::io::Error),

    #[error("Format failure")]
    Format(#[from] stone::read::Error),
}

struct ProgressWriter<W> {
    writer: W,
    total: u64,
    written: u64,
    progress: ProgressBar,
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
        self.progress
            .set_position((self.written as f64 / self.total as f64 * 1000.0) as u64);

        Ok(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
