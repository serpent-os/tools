// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::process;

use container::Container;
use stone_recipe::Recipe;

use crate::Cache;

pub fn chroot(recipe: &Recipe, cache: &Cache) -> Result<(), container::Error> {
    run(recipe, cache, || {
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
    recipe: &Recipe,
    cache: &Cache,
    f: impl FnMut() -> Result<(), container::Error>,
) -> Result<(), container::Error> {
    let rootfs = cache.rootfs().host;

    let artefacts_cache = cache.artefacts();
    let build_cache = cache.build();
    let compiler_cache = cache.ccache();

    Container::new(rootfs)
        .hostname("boulder")
        .networking(recipe.options.networking)
        .work_dir(&build_cache.guest)
        .bind(&artefacts_cache.host, &artefacts_cache.guest)
        .bind(&build_cache.host, &build_cache.guest)
        .bind(&compiler_cache.host, &compiler_cache.guest)
        .run(f)
}
