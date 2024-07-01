// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Boot management integration in moss

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use fnmatch::Pattern;
use stone::payload::{layout, Layout};
use thiserror::{self, Error};

use crate::package::Id;

#[derive(Debug, Error)]
pub enum Error {
    #[error("blsforme: {0}")]
    Blsforme(#[from] blsforme::Error),

    /// fnmatch pattern compilation for boot, etc.
    #[error("fnmatch pattern: {0}")]
    Pattern(#[from] fnmatch::Error),

    #[error("incomplete kernel tree: {0}")]
    IncompleteKernel(String),
}

/// Simple mapping type for kernel discovery paths, retaining the layout reference
#[derive(Debug)]
struct KernelCandidate<'a> {
    path: PathBuf,
    _layout: &'a Layout,
}

impl<'a> AsRef<Path> for KernelCandidate<'a> {
    fn as_ref(&self) -> &Path {
        self.path.as_path()
    }
}

/// From a given set of input paths, produce a set of match pairs
fn kernel_files_from_state<'a>(layouts: &'a [(Id, Layout)], pattern: &'a Pattern) -> Vec<KernelCandidate<'a>> {
    let mut kernel_entries = vec![];

    for (_, path) in layouts.iter() {
        match &path.entry {
            layout::Entry::Regular(_, target) => {
                if pattern.match_path(target).is_some() {
                    kernel_entries.push(KernelCandidate {
                        path: PathBuf::from(target),
                        _layout: path,
                    });
                }
            }
            layout::Entry::Symlink(_, target) => {
                if pattern.match_path(target).is_some() {
                    kernel_entries.push(KernelCandidate {
                        path: PathBuf::from(target),
                        _layout: path,
                    });
                }
            }
            _ => {}
        }
    }

    kernel_entries
}

pub fn synchronize(root: impl AsRef<Path>, layouts: &[(Id, Layout)]) -> Result<(), Error> {
    let root = root.as_ref();
    let is_native = root.to_string_lossy() == "/";
    // Create an appropriate configuration
    let config = blsforme::Configuration {
        root: if is_native {
            blsforme::Root::Native(root.into())
        } else {
            blsforme::Root::Image(root.into())
        },
        vfs: root.into(),
    };

    let pattern = fnmatch::Pattern::from_str("lib/kernel/(version:*)/*")?;

    // No kernels? No bother.
    let kernels = kernel_files_from_state(layouts, &pattern);
    if kernels.is_empty() {
        return Ok(());
    }
    let schema = blsforme::Schema::Blsforme;
    let discovered = schema.discover_system_kernels(kernels.iter())?;

    eprintln!("Discovered kernels in current state: ");
    discovered.iter().for_each(|p| eprintln!("kernel = {p:?}"));

    // Init the manager
    let _ = blsforme::Manager::new(&config)?.with_kernels(discovered);

    Ok(())
}
