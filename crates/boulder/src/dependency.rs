// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;

use stone_recipe::{Recipe, Upstream};

pub fn calculate(recipe: &Recipe, ccache: bool) -> Vec<&str> {
    let mut packages = BASE_PACKAGES.to_vec();

    match recipe.options.toolchain {
        stone_recipe::Toolchain::Llvm => packages.extend(LLVM_PACKAGES),
        stone_recipe::Toolchain::Gnu => packages.extend(GNU_PACKAGES),
    }

    if recipe.emul32 {
        packages.extend(BASE32_PACKAGES);

        match recipe.options.toolchain {
            stone_recipe::Toolchain::Llvm => packages.extend(LLVM32_PACKAGES),
            stone_recipe::Toolchain::Gnu => packages.extend(GNU32_PACKAGES),
        }
    }

    if ccache {
        packages.push(CCACHE_PACKAGE);
    }

    packages.extend(recipe.build.build_deps.iter().map(String::as_str));
    packages.extend(recipe.build.check_deps.iter().map(String::as_str));

    for upstream in &recipe.upstreams {
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

    packages
        .into_iter()
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
];
const BASE32_PACKAGES: &[&str] = &["glibc-32bit-devel"];

const GNU_PACKAGES: &[&str] = &["binutils", "gcc-devel"];
const GNU32_PACKAGES: &[&str] = &["gcc-32bit-devel"];

const LLVM_PACKAGES: &[&str] = &["clang"];
const LLVM32_PACKAGES: &[&str] = &["clang-32bit", "libcxx-32bit-devel"];

const CCACHE_PACKAGE: &str = "binary(ccache)";
