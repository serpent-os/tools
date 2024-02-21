// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    fs::{self, File},
    io::{self, Write},
};

use itertools::Itertools;
use moss::{package::Meta, stone, Dependency};
use thiserror::Error;

use self::manifest::Manifest;
use super::collect::PathInfo;
use crate::{architecture, Architecture, Paths, Recipe};

mod manifest;

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub build_release: u64,
    pub architecture: Architecture,
    pub source: stone_recipe::Source,
    pub definition: stone_recipe::Package,
    pub paths: Vec<PathInfo>,
}

impl Package {
    pub fn new(
        name: String,
        source: stone_recipe::Source,
        template: stone_recipe::Package,
        paths: Vec<PathInfo>,
    ) -> Self {
        Self {
            name,
            build_release: 1,
            architecture: architecture::host(),
            source,
            definition: template,
            paths,
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
            name: self.name.clone().into(),
            version_identifier: self.source.version.clone(),
            source_release: self.source.release,
            build_release: self.build_release,
            architecture: self.architecture.to_string(),
            summary: self.definition.summary.clone().unwrap_or_default(),
            description: self.definition.description.clone().unwrap_or_default(),
            source_id: self.source.name.clone(),
            homepage: self.source.homepage.clone(),
            licenses: self.source.license.clone().into_iter().sorted().collect(),
            // TODO: Deps from analyzer
            dependencies: self
                .definition
                .run_deps
                .clone()
                .into_iter()
                .filter_map(|dep| dep.parse::<Dependency>().ok())
                .sorted_by_key(|dep| dep.to_string())
                .collect(),
            // TODO: Providers from analyzer
            providers: Default::default(),
            uri: None,
            hash: None,
            download_size: None,
        }
    }
}

pub fn emit(paths: &Paths, recipe: &Recipe, packages: &[Package]) -> Result<(), Error> {
    let mut manfiest = Manifest::new(paths, recipe, architecture::host());

    for package in packages {
        if !package.is_dbginfo() {
            manfiest.add_package(package);
        }

        emit_package(paths, package)?;
    }

    manfiest.write_binary()?;
    manfiest.write_json()?;

    Ok(())
}

fn emit_package(paths: &Paths, package: &Package) -> Result<(), Error> {
    let filename = package.filename();

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
        let layouts = package.paths.iter().map(|p| p.layout.clone()).collect::<Vec<_>>();
        writer.add_payload(layouts.as_slice())?;
    }

    // Temp file for building content payload
    let temp_content_path = format!("/tmp/{}.tmp", &filename);
    let mut temp_content = File::options()
        .read(true)
        .append(true)
        .create(true)
        .open(&temp_content_path)?;

    // Sort all files by size, largest to smallest
    let files = package
        .paths
        .iter()
        .filter(|p| p.is_file())
        .sorted_by(|a, b| a.size.cmp(&b.size).reverse())
        .collect::<Vec<_>>();

    // Convert to content writer using pledged size = total size of all files
    let pledged_size = files.iter().map(|p| p.size).sum();
    let mut writer = writer.with_content(&mut temp_content, Some(pledged_size))?;

    // Add each file content
    for info in files {
        let mut file = File::open(&info.path)?;

        writer.add_content(&mut file)?;
    }

    // Finalize & flush
    writer.finalize()?;
    out_file.flush()?;

    // Remove temp content file
    fs::remove_file(temp_content_path)?;

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
