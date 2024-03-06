// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;
use std::io;

use moss::{repository, runtime, Installation};
use stone_recipe::{tuning::Toolchain, Upstream};
use thiserror::Error;

use crate::build::Builder;
use crate::{container, util};

pub fn populate(builder: &Builder, repositories: repository::Map) -> Result<(), Error> {
    let packages = packages(builder);

    let rootfs = builder.paths.rootfs().host;

    // Recreate root
    util::recreate_dir(&rootfs)?;

    // Create the moss client
    let installation = Installation::open(&builder.env.moss_dir)?;
    let mut moss_client =
        moss::Client::with_explicit_repositories("boulder", installation, repositories)?.ephemeral(&rootfs)?;

    // Ensure all configured repos have been initialized (important since users
    // might add profile configs from an editor)
    runtime::block_on(moss_client.ensure_repos_initialized())?;

    // Install packages
    moss_client.install(&packages, true)?;

    Ok(())
}

pub fn clean(builder: &Builder) -> Result<(), Error> {
    // Dont't need to clean if it doesn't exist
    if !builder.paths.build().host.exists() {
        return Ok(());
    }

    // We recreate inside the container so we don't
    // get permissions error if this is a rootless build
    // and there's subuid mappings into the user namespace
    container::exec(&builder.paths, false, || {
        // Recreate `install` dir
        util::recreate_dir(&builder.paths.install().guest)?;

        for target in &builder.targets {
            for job in &target.jobs {
                // Recerate build dir
                util::recreate_dir(&job.build_dir)?;
            }
        }

        Ok(()) as Result<_, io::Error>
    })?;

    Ok(())
}

fn packages(builder: &Builder) -> Vec<&str> {
    let mut packages = BASE_PACKAGES.to_vec();

    match builder.recipe.parsed.options.toolchain {
        Toolchain::Llvm => packages.extend(LLVM_PACKAGES),
        Toolchain::Gnu => packages.extend(GNU_PACKAGES),
    }

    if builder.recipe.parsed.emul32 {
        packages.extend(BASE32_PACKAGES);

        match builder.recipe.parsed.options.toolchain {
            Toolchain::Llvm => packages.extend(LLVM32_PACKAGES),
            Toolchain::Gnu => packages.extend(GNU32_PACKAGES),
        }
    }

    if builder.ccache {
        packages.push(CCACHE_PACKAGE);
    }

    packages.extend(builder.recipe.parsed.build.build_deps.iter().map(String::as_str));
    packages.extend(builder.recipe.parsed.build.check_deps.iter().map(String::as_str));

    for upstream in &builder.recipe.parsed.upstreams {
        if let Upstream::Plain { uri, .. } = upstream {
            let path = uri.path();

            if let Some((_, ext)) = path.rsplit_once('.') {
                match ext {
                    "xz" => {
                        packages.extend(["binary(tar)", "binary(xz)"]);
                    }
                    "zst" => {
                        packages.extend(["binary(tar)", "binary(zstd)"]);
                    }
                    "bz2" => {
                        packages.extend(["binary(tar)", "binary(bzip2)"]);
                    }
                    "gz" => {
                        packages.extend(["binary(tar)", "binary(gzip)"]);
                    }
                    "zip" => {
                        packages.push("binary(unzip)");
                    }
                    "rpm" => {
                        packages.extend(["binary(rpm2cpio)", "cpio"]);
                    }
                    "deb" => {
                        packages.push("binary(ar)");
                    }
                    _ => {}
                }
            }
        }
    }

    // Dependencies from all scripts in the builder
    let extra_deps = builder.extra_deps();

    packages
        .into_iter()
        .chain(extra_deps)
        // Remove dupes
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

const BASE_PACKAGES: &[&str] = &[
    "bash",
    "boulder",
    "coreutils",
    "dash",
    "diffutils",
    "findutils",
    "gawk",
    "glibc-devel",
    "grep",
    "libarchive",
    "linux-headers",
    "pkgconf",
    "sed",
    "util-linux",
    // Needed for chroot
    "binary(git)",
    "binary(nano)",
    "binary(vim)",
    "binary(ps)",
];
const BASE32_PACKAGES: &[&str] = &["glibc-32bit-devel"];

const GNU_PACKAGES: &[&str] = &["binutils", "gcc-devel"];
const GNU32_PACKAGES: &[&str] = &["gcc-32bit-devel"];

const LLVM_PACKAGES: &[&str] = &["clang"];
const LLVM32_PACKAGES: &[&str] = &["clang-32bit", "libcxx-32bit-devel"];

const CCACHE_PACKAGE: &str = "binary(ccache)";

#[derive(Debug, Error)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("moss client")]
    MossClient(#[from] moss::client::Error),
    #[error("moss install")]
    MossInstall(#[from] moss::client::install::Error),
    #[error("moss installation")]
    MossInstallation(#[from] moss::installation::Error),
    #[error("container")]
    Container(#[from] container::Error),
}
