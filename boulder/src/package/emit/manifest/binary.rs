// SPDX-FileCopyrightText: Copyright © 2020-2025 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::BTreeSet, path::Path};

use fs_err::File;
use moss::Dependency;
use stone::{
    header::v1::FileType,
    payload::{self, meta},
};

use super::Error;
use crate::package::emit::Package;

pub fn write(path: &Path, packages: &BTreeSet<&Package<'_>>, build_deps: &BTreeSet<String>) -> Result<(), Error> {
    let mut output = File::create(path)?;

    let mut writer = stone::Writer::new(&mut output, FileType::BuildManifest)?;

    // Add each package
    for package in packages {
        let mut meta = package.meta();
        // deliberately override .stone package metadata and set build_release to zero for binary manifests
        meta.build_release = 0;
        let mut payload = meta.to_stone_payload();

        // Add build deps
        for name in build_deps {
            if let Ok(dep) = Dependency::from_name(name) {
                payload.push(payload::Meta {
                    tag: meta::Tag::BuildDepends,
                    kind: meta::Kind::Dependency(dep.kind.into(), dep.name),
                });
            }
        }

        writer.add_payload(payload.as_slice())?;
    }

    writer.finalize()?;

    Ok(())
}
