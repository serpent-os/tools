// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::process;

use container::Container;

use crate::Cache;

pub fn chroot(cache: &Cache) -> Result<(), container::Error> {
    run(cache, || {
        let mut child = process::Command::new("/bin/bash")
            .arg("--login")
            .env_clear()
            .env("HOME", &cache.build().guest)
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .spawn()?;

        child.wait()?;

        Ok(())
    })
}

fn run(
    cache: &Cache,
    f: impl FnMut() -> Result<(), container::Error>,
) -> Result<(), container::Error> {
    let rootfs = cache.rootfs().host;

    let artefacts_cache = cache.artefacts();
    let build_cache = cache.build();
    let compiler_cache = cache.ccache();

    Container::new(rootfs)
        .hostname("boulder")
        .work_dir(&build_cache.guest)
        .bind(&artefacts_cache.host, &artefacts_cache.guest)
        .bind(&build_cache.host, &build_cache.guest)
        .bind(&compiler_cache.host, &compiler_cache.guest)
        .run(f)
}
