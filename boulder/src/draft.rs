// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;
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
        let extract_root = PathBuf::from("/tmp/boulder-new");

        // Fetch and extract all upstreams
        let extracted = upstream::fetch_and_extract(&self.upstreams, &extract_root)?;

        // Build metadata from extracted upstreams
        let metadata = Metadata::new(extracted);

        // Enumerate all extracted files
        let files = util::enumerate_files(&extract_root, |_| true)?
            .into_iter()
            .map(|path| File {
                path,
                extract_root: &extract_root,
            })
            .collect::<Vec<_>>();

        // Analyze files to determine build system / collect deps
        let build = build::analyze(&files).map_err(Error::AnalyzeBuildSystem)?;

        // Remove temp extract dir
        fs::remove_dir_all(extract_root)?;

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
        format!("builddeps   :\n{deps}\n")
    }
}

pub struct File<'a> {
    pub path: PathBuf,
    pub extract_root: &'a Path,
}

impl<'a> File<'a> {
    // The depth of a file relative to it's extracted archive
    pub fn depth(&self) -> usize {
        let relative = self.path.strip_prefix(self.extract_root).unwrap_or(&self.path);

        // Subtract 2 so root of archive folder == depth 0
        relative.iter().count().saturating_sub(2)
    }

    pub fn file_name(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or_default()
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

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_file_depth() {
        let extract_root = Path::new("/tmp/test");

        let file = File {
            path: PathBuf::from("/tmp/test/some_archive/meson.build"),
            extract_root,
        };

        assert_eq!(file.depth(), 0);
    }
}
