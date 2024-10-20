// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    io::{self, Write},
    num::NonZeroU64,
    time::Duration,
};

use fs_err::{self as fs, File};
use itertools::Itertools;
use moss::{package::Meta, Dependency, Provider};
use thiserror::Error;
use tui::{ProgressBar, ProgressStyle, Styled};

use self::manifest::Manifest;
use super::analysis;
use crate::{architecture, util, Architecture, Paths, Recipe};

mod manifest;

#[derive(Debug)]
pub struct Package<'a> {
    pub name: &'a str,
    pub build_release: NonZeroU64,
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
        build_release: NonZeroU64,
    ) -> Self {
        Self {
            name,
            architecture: architecture::host(),
            source,
            definition: template,
            analysis,
            build_release,
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
            build_release: self.build_release.get(),
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
            conflicts: self
                .definition
                .conflicts
                .iter()
                .filter_map(|name| Provider::from_name(name).ok())
                .collect(),
            uri: None,
            hash: None,
            download_size: None,
        }
    }
}

impl PartialEq for Package<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(other.name)
    }
}

impl Eq for Package<'_> {}

impl PartialOrd for Package<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Package<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(other.name)
    }
}

pub fn emit(paths: &Paths, recipe: &Recipe, packages: &[Package]) -> Result<(), Error> {
    let mut manifest = Manifest::new(paths, recipe, architecture::host());

    println!("Packaging");

    for package in packages {
        if !package.is_dbginfo() {
            manifest.add_package(package);
        }

        emit_package(paths, package)?;
    }

    manifest.write_binary()?;
    manifest.write_json()?;

    println!();

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
        if !layouts.is_empty() {
            writer.add_payload(layouts.as_slice())?;
        }
    }

    // Only add content payload if we have some files
    if !sorted_files.is_empty() {
        // Temp file for building content payload
        let temp_content_path = format!("/tmp/{}.tmp", &filename);
        let mut temp_content = fs::OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&temp_content_path)?;

        // Convert to content writer using pledged size = total size of all files
        let mut writer =
            writer.with_content(&mut temp_content, Some(total_file_size), util::num_cpus().get() as u32)?;

        for file in sorted_files {
            let file = File::open(&file.path)?;
            writer.add_content(&mut pb.wrap_read(&file))?;
        }

        // Finalize & flush
        writer.finalize()?;
        out_file.flush()?;

        // Remove temp content file
        fs::remove_file(temp_content_path)?;
    } else {
        // Finalize & flush
        writer.finalize()?;
        out_file.flush()?;
    }

    pb.suspend(|| println!("{} {filename}", "Emitted".green()));
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
