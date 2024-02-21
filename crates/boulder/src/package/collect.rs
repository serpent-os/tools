// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    fs::{self, Metadata},
    io,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use glob::Pattern;
use moss::stone::payload::{layout, Layout};
use moss::stone::write::digest;
use nix::libc::{S_IFDIR, S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Rule {
    pub pattern: String,
    pub package: String,
}

impl Rule {
    pub fn matches(&self, path: &str) -> bool {
        self.pattern == path
            || path.starts_with(&self.pattern)
            || Pattern::new(&self.pattern)
                .map(|pattern| pattern.matches(path))
                .unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct Collector {
    /// Rules stored in order of
    /// ascending priority
    rules: Vec<Rule>,
}

impl Collector {
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn matching_package(&self, path: &str) -> Option<&str> {
        // Rev = check highest priority rules first
        self.rules
            .iter()
            .rev()
            .find_map(|rule| rule.matches(path).then_some(rule.package.as_str()))
    }

    pub fn paths(
        &self,
        root: &Path,
        subdir: Option<(PathBuf, Metadata)>,
        hasher: &mut digest::Hasher,
    ) -> Result<Vec<(String, PathInfo)>, io::Error> {
        let mut paths = vec![];

        let add_path =
            |path: PathBuf, metadata: Metadata, paths: &mut Vec<(String, PathInfo)>, hasher: &mut digest::Hasher| {
                let target_path = Path::new("/").join(path.strip_prefix(root).expect("path is ancestor of root"));

                if let Some(package) = self.matching_package(target_path.to_str().unwrap_or_default()) {
                    paths.push((package.to_string(), PathInfo::new(path, target_path, metadata, hasher)?))
                }

                Ok(()) as Result<(), io::Error>
            };

        let dir = subdir.as_ref().map(|t| t.0.as_path()).unwrap_or(root);
        let entries = fs::read_dir(dir)?;

        for result in entries {
            let entry = result?;
            let metadata = entry.metadata()?;

            let host_path = entry.path();

            if metadata.is_dir() {
                paths.extend(self.paths(root, Some((host_path, metadata)), hasher)?);
            } else {
                add_path(host_path, metadata, &mut paths, hasher)?;
            }
        }

        // Include empty or special dir
        //
        // Regular 755 dir w/ entries can be
        // recreated when adding children
        if let Some((dir, meta)) = subdir {
            const REGULAR_DIR_MODE: u32 = S_IFDIR | S_IROTH | S_IXOTH | S_IRGRP | S_IXGRP | S_IRWXU;

            let is_special = meta.mode() != REGULAR_DIR_MODE;

            if meta.is_dir() && (paths.is_empty() || is_special) {
                add_path(dir, meta, &mut paths, hasher)?;
            }
        }

        Ok(paths)
    }
}

#[derive(Debug)]
pub struct PathInfo {
    pub path: PathBuf,
    pub layout: Layout,
    pub size: u64,
}

impl PathInfo {
    pub fn new(
        path: PathBuf,
        target_path: PathBuf,
        metadata: Metadata,
        hasher: &mut digest::Hasher,
    ) -> Result<Self, io::Error> {
        // Strip /usr prefix
        let target = target_path
            .strip_prefix("/usr")
            .unwrap_or(&target_path)
            .to_string_lossy()
            .to_string();

        let file_type = metadata.file_type();

        let layout = Layout {
            uid: metadata.uid(),
            gid: metadata.gid(),
            mode: metadata.mode(),
            tag: 0,
            entry: if file_type.is_symlink() {
                let source = fs::read_link(&path)?;

                layout::Entry::Symlink(source.to_string_lossy().to_string(), target)
            } else if file_type.is_dir() {
                layout::Entry::Directory(target)
            } else if file_type.is_char_device() {
                layout::Entry::CharacterDevice(target)
            } else if file_type.is_block_device() {
                layout::Entry::BlockDevice(target)
            } else if file_type.is_fifo() {
                layout::Entry::Fifo(target)
            } else if file_type.is_socket() {
                layout::Entry::Socket(target)
            } else {
                hasher.reset();

                let mut digest_writer = digest::Writer::new(io::sink(), hasher);
                let mut file = fs::File::open(&path)?;

                // Copy bytes to null sink so we don't
                // explode memory
                io::copy(&mut file, &mut digest_writer)?;

                let hash = hasher.digest128();

                layout::Entry::Regular(hash, target)
            },
        };

        Ok(Self {
            path,
            layout,
            size: metadata.size(),
        })
    }

    pub fn is_file(&self) -> bool {
        matches!(self.layout.entry, layout::Entry::Regular(_, _))
    }
}
