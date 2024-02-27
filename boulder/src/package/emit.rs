// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    fs::{self, File},
    io::{self, Write},
    time::Duration,
};

use itertools::Itertools;
use moss::{package::Meta, stone, Dependency};
use thiserror::Error;
use tui::{ProgressBar, ProgressReader, ProgressStyle, Stylize};

use self::manifest::Manifest;
use super::analysis;
use crate::{architecture, Architecture, Paths, Recipe};

mod manifest;

#[derive(Debug)]
pub struct Package<'a> {
    pub name: &'a str,
    pub build_release: u64,
    pub architecture: Architecture,
    pub source: &'a stone_recipe::Source,
    pub definition: &'a stone_recipe::Package,
    pub analysis: analysis::Bucket,
}

impl<'a> Package<'a> {
    pub fn new(
        name: &'a str,
        source: &'a stone_recipe::Source,
        template: &'a stone_recipe::Package,
        analysis: analysis::Bucket,
    ) -> Self {
        Self {
            name,
            build_release: 1,
            architecture: architecture::host(),
            source,
            definition: template,
            analysis,
        }
    }

    pub fn is_dbginfo(&self) -> bool {
        self.name.ends_with("-dbginfo")
    }

    pub fn filename(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}.stone",
            self.name, self.source.version, self.source.release, self.build_release, self.architecture
        )
    }

    pub fn meta(&self) -> Meta {
        Meta {
            name: self.name.to_string().into(),
            version_identifier: self.source.version.clone(),
            source_release: self.source.release,
            build_release: self.build_release,
            architecture: self.architecture.to_string(),
            summary: self.definition.summary.clone().unwrap_or_default(),
            description: self.definition.description.clone().unwrap_or_default(),
            source_id: self.source.name.clone(),
            homepage: self.source.homepage.clone(),
            licenses: self.source.license.clone().into_iter().sorted().collect(),
            dependencies: self
                .analysis
                .dependencies()
                .cloned()
                .chain(
                    self.definition
                        .run_deps
                        .iter()
                        .filter_map(|name| Dependency::from_name(name).ok()),
                )
                .collect(),
            providers: self.analysis.providers().cloned().collect(),
            uri: None,
            hash: None,
            download_size: None,
        }
    }
}

pub fn emit(paths: &Paths, recipe: &Recipe, packages: &[Package]) -> Result<(), Error> {
    let mut manifest = Manifest::new(paths, recipe, architecture::host());

    println!("Emitting packages\n");

    for package in packages {
        if !package.is_dbginfo() {
            manifest.add_package(package);
        }

        emit_package(paths, package)?;
    }

    manifest.write_binary()?;
    manifest.write_json()?;

    Ok(())
}

fn emit_package(paths: &Paths, package: &Package) -> Result<(), Error> {
    let filename = package.filename();

    // Sort all files by size, largest to smallest
    let sorted_files = package
        .analysis
        .paths
        .iter()
        .filter(|p| p.is_file())
        .sorted_by(|a, b| a.size.cmp(&b.size).reverse())
        .collect::<Vec<_>>();
    let total_file_size = sorted_files.iter().map(|p| p.size).sum();

    let pb = ProgressBar::new(total_file_size)
        .with_message(format!("Generating {filename}"))
        .with_style(
            ProgressStyle::with_template(" {spinner} |{percent:>3}%| {wide_msg} {binary_bytes_per_sec:>.dim} ")
                .unwrap()
                .tick_chars("--=≡■≡=--"),
        );
    pb.enable_steady_tick(Duration::from_millis(150));

    // Output file to artefacts directory
    let out_path = paths.artefacts().guest.join(&filename);
    if out_path.exists() {
        fs::remove_file(&out_path)?;
    }
    let mut out_file = File::create(out_path)?;

    // Create stone binary writer
    let mut writer = stone::Writer::new(&mut out_file, stone::header::v1::FileType::Binary)?;

    // Add metadata
    {
        let meta = package.meta();
        writer.add_payload(meta.to_stone_payload().as_slice())?;
    }

    // Add layouts
    {
        let layouts = package
            .analysis
            .paths
            .iter()
            .map(|p| p.layout.clone())
            .collect::<Vec<_>>();
        writer.add_payload(layouts.as_slice())?;
    }

    // Temp file for building content payload
    let temp_content_path = format!("/tmp/{}.tmp", &filename);
    let mut temp_content = File::options()
        .read(true)
        .append(true)
        .create(true)
        .open(&temp_content_path)?;

    // Convert to content writer using pledged size = total size of all files
    let mut writer = writer.with_content(&mut temp_content, Some(total_file_size))?;

    let mut total_read = 0;

    // Add each file content
    for file in sorted_files {
        let mut file = File::open(&file.path)?;
        let mut progress_reader = ProgressReader {
            reader: &mut file,
            total: total_file_size,
            read: total_read,
            progress: pb.clone(),
        };

        writer.add_content(&mut progress_reader)?;

        total_read = progress_reader.read;
    }

    // Finalize & flush
    writer.finalize()?;
    out_file.flush()?;

    // Remove temp content file
    fs::remove_file(temp_content_path)?;

    pb.println(format!("{} {filename}", "Emitted".green()));
    pb.finish_and_clear();

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("stone binary writer")]
    StoneBinaryWriter(#[from] stone::write::Error),
    #[error("manifest")]
    Manifest(#[from] manifest::Error),
    #[error("io")]
    Io(#[from] io::Error),
}