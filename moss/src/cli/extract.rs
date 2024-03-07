// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fs::{create_dir_all, hard_link, remove_dir_all, remove_file, File},
    io::{copy, Read, Seek, SeekFrom},
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

use clap::{arg, ArgMatches, Command};
use color_eyre::{
    eyre::{eyre, Context},
    Result, Section,
};
use moss::package;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use stone::{payload::layout, read::PayloadKind};
use tui::{ProgressBar, ProgressStyle};

pub fn command() -> Command {
    Command::new("extract")
        .about("Extract a `.stone` content to disk")
        .long_about("For all valid content-bearing archives, extract to disk")
        .arg(arg!(<PATH> ... "files to inspect").value_parser(clap::value_parser!(PathBuf)))
}

/// Handle the `extract` command
pub fn handle(args: &ArgMatches) -> Result<()> {
    let paths = args
        .get_many::<PathBuf>("PATH")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    let content_store = Path::new(".stoneStore");

    // Begin unpack
    create_dir_all(content_store)
        .context("create temporary extract directory")
        .suggestion("is the current directory writable?")?;

    for path in paths {
        extract(&path, content_store).with_context(|| eyre!("extract {path:?}"))?;
    }

    // Clean up.
    remove_dir_all(content_store).context("remove temporary extract directory")?;

    Ok(())
}

fn extract(path: &Path, content_store: &Path) -> Result<()> {
    println!("Extract: {:?}", path);

    let rdr = File::open(path)
        .context("open file")
        .suggestion("does the file exist?")?;
    let mut reader = stone::read(rdr)
        .context("read stone file")
        .suggestion("is this a valid stone file?")?;

    let payloads = reader
        .payloads()
        .context("seeking payloads")?
        .collect::<Result<Vec<_>, _>>()
        .context("decode payload")?;
    let content = payloads.iter().find_map(PayloadKind::content);
    let layouts = payloads.iter().find_map(PayloadKind::layout);
    let meta = payloads
        .iter()
        .find_map(PayloadKind::meta)
        .ok_or_else(|| eyre!("missing metadata payload"))?;

    let pkg = package::Meta::from_stone_payload(&meta.body).context("metadata payload is malformed")?;
    let extraction_root = PathBuf::from(pkg.id().to_string());

    // Cleanup old extraction root
    if extraction_root.exists() {
        remove_dir_all(&extraction_root).context("remove temporary stone extract directory")?;
    }

    if let Some(content) = content {
        let content_file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(".stoneContent")
            .context("open temporary content extract file")?;

        let progress = ProgressBar::new(content.header.plain_size).with_style(
            ProgressStyle::with_template("|{bar:20.cyan/bue}| {percent}%")
                .unwrap()
                .progress_chars("■≡=- "),
        );
        reader
            .unpack_content(content, &mut progress.wrap_write(&content_file))
            .context("unpacking stone content payload")?;

        // Extract all indices from the `.stoneContent` into hash-indexed unique files
        payloads
            .par_iter()
            .filter_map(PayloadKind::index)
            .flat_map(|p| &p.body)
            .map(|idx| {
                // Split file reader over index range
                let mut file = &content_file;
                file.seek(SeekFrom::Start(idx.start))
                    .with_context(|| eyre!("seek to byte {}", idx.start))?;
                let mut split_file = (&mut file).take(idx.end - idx.start);

                let mut output = File::create(format!(".stoneStore/{:02x}", idx.digest))
                    .with_context(|| eyre!("create output file .stoneStore/{:02x}", idx.digest))?;

                copy(&mut split_file, &mut output).with_context(|| eyre!("copy bytes {} to {}", idx.start, idx.end))?;

                Ok(())
            })
            .collect::<Result<Vec<_>>>()
            .context("unpack file from content payload")?;

        remove_file(".stoneContent").context("remove temporary content extract file")?;
    }

    if let Some(layouts) = layouts {
        for layout in &layouts.body {
            match &layout.entry {
                layout::Entry::Regular(id, target) => {
                    let store_path = content_store.join(format!("{:02x}", id));
                    let target_disk = extraction_root.join("usr").join(target);

                    // drop it into a valid dir
                    // TODO: Fix the permissions & mask
                    let directory_target = target_disk.parent().unwrap();
                    create_dir_all(directory_target).context("create extract directory")?;

                    // link from CA store
                    hard_link(&store_path, &target_disk)
                        .with_context(|| eyre!("hardlink from {store_path:?} to {target_disk:?}"))?;
                }
                layout::Entry::Symlink(source, target) => {
                    let target_disk = extraction_root.join("usr").join(target);
                    let directory_target = target_disk.parent().unwrap();

                    // ensure dumping ground exists
                    create_dir_all(directory_target).context("create extract directory")?;

                    // join the link path to the directory target for relative joinery
                    symlink(source, &target_disk)
                        .with_context(|| eyre!("hardlink from {source:?} to {target_disk:?}"))?;
                }
                layout::Entry::Directory(target) => {
                    let target_disk = extraction_root.join("usr").join(target);
                    // TODO: Fix perms!
                    create_dir_all(target_disk).context("create extract directory")?;
                }
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
