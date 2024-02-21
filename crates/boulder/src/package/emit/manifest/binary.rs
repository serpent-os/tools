// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::BTreeSet, fs::File, path::Path};

use moss::{
    dependency,
    stone::{
        self,
        header::v1::FileType,
        payload::{self, meta},
    },
    Dependency,
};

use super::Error;
use crate::package::emit::Package;

pub fn write(path: &Path, packages: &[&Package], build_deps: &BTreeSet<String>) -> Result<(), Error> {
    let mut output = File::create(path)?;

    let mut writer = stone::Writer::new(&mut output, FileType::BuildManifest)?;

    // Add each package
    for package in packages {
        let mut payload = package.meta().to_stone_payload();

        // Add build deps
        for name in build_deps {
            let dep = name.parse::<Dependency>().unwrap_or_else(|_| Dependency {
                kind: dependency::Kind::PackageName,
                name: name.to_string(),
            });

            payload.push(payload::Meta {
                tag: meta::Tag::BuildDepends,
                kind: meta::Kind::Dependency(dep.kind.into(), dep.name),
            });
        }

        writer.add_payload(payload.as_slice())?;
    }

    writer.finalize()?;

    Ok(())
}
