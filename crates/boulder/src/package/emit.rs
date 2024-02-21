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

use super::collect::PathInfo;
use crate::{architecture, Architecture, Paths};

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

    pub fn filename(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}.stone",
            self.name, self.source.version, self.source.release, self.build_release, self.architecture
        )
    }
}

// TODO: Add binary & json manifest
pub fn emit(paths: &Paths, packages: Vec<Package>) -> Result<(), Error> {
    for package in packages {
        emit_package(paths, package)?;
    }

    Ok(())
}

fn emit_package(paths: &Paths, package: Package) -> Result<(), Error> {
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
        let metadata = Meta {
            name: package.name.into(),
            version_identifier: package.source.version,
            source_release: package.source.release,
            build_release: package.build_release,
            architecture: package.architecture.to_string(),
            summary: package.definition.summary.unwrap_or_default(),
            description: package.definition.description.unwrap_or_default(),
            source_id: package.source.name,
            homepage: package.source.homepage,
            licenses: package.source.license.into_iter().sorted().collect(),
            // TODO: Deps from analyzer
            dependencies: package
                .definition
                .run_deps
                .into_iter()
                .filter_map(|dep| dep.parse::<Dependency>().ok())
                .sorted_by_key(|dep| dep.to_string())
                .collect(),
            // TODO: Providers from analyzer
            providers: Default::default(),
            uri: None,
            hash: None,
            download_size: None,
        };
        writer.add_payload(metadata.to_stone_payload().as_slice())?;
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
        .into_iter()
        .filter(|p| p.is_file())
        .sorted_by(|a, b| a.size.cmp(&b.size).reverse())
        .collect::<Vec<_>>();

    // Convert to content writer using pledged size = total size of all files
    let pledged_size = files.iter().map(|p| p.size).sum();
    let mut writer = writer.with_content(&mut temp_content, Some(pledged_size))?;

    // Add each file content
    for info in files {
        let mut file = File::open(info.path)?;

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
    #[error("io")]
    Io(#[from] io::Error),
}
