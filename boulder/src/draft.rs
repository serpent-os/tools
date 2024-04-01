// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fs, io, path::PathBuf};

use itertools::Itertools;
use moss::Dependency;
use thiserror::Error;
use url::Url;

use crate::util;

use self::metadata::Metadata;
use self::upstream::Upstream;

mod build;
mod metadata;
mod upstream;

pub struct Drafter {
    upstreams: Vec<Url>,
}

impl Drafter {
    pub fn new(upstreams: Vec<Url>) -> Self {
        Self { upstreams }
    }

    pub fn run(&self) -> Result<String, Error> {
        // TODO: Use tempdir
        let extract_dir = PathBuf::from("/tmp/boulder-new");

        // Fetch and extract all upstreams
        let extracted = upstream::fetch_and_extract(&self.upstreams, &extract_dir)?;

        // Build metadata from extracted upstreams
        let metadata = Metadata::new(extracted);

        // Enumerate all extracted files
        let files = util::enumerate_files(&extract_dir, |_| true)?;

        // Analyze files to determine build system / collect deps
        let build = build::analyze(&files).map_err(Error::AnalyzeBuildSystem)?;

        // Remove temp extract dir
        fs::remove_dir_all(extract_dir)?;

        let Some(build_system) = build.detected_system else {
            return Err(Error::UnhandledBuildSystem);
        };

        let builddeps = builddeps(build.dependencies);
        let phases = build_system.phases();

        #[rustfmt::skip]
        let template = format!(
"#
# SPDX-FileCopyrightText: © 2020-2024 Serpent OS Developers
#
# SPDX-License-Identifier: MPL-2.0
#                
name        : {}
version     : {}
release     : 1
homepage    : {}
upstreams   :
{}
summary     : UPDATE SUMMARY
description : |
    UPDATE DESCRIPTION
license     : UPDATE LICENSE
{builddeps}{phases}
",
            metadata.source.name,
            metadata.source.version,
            metadata.source.homepage,
            metadata.upstreams(),
        );

        Ok(template)
    }
}

fn builddeps(deps: impl IntoIterator<Item = Dependency>) -> String {
    let deps = deps.into_iter().map(|dep| format!("    - {dep}")).sorted().join("\n");

    if deps.is_empty() {
        String::default()
    } else {
        format!("builddeps   :\n{deps}")
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unhandled build system")]
    UnhandledBuildSystem,
    #[error("analyzing build system")]
    AnalyzeBuildSystem(#[source] build::Error),
    #[error("upstream")]
    Upstream(#[from] upstream::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
