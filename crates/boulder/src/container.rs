// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::process;

use container::Container;

use crate::Job;

pub fn chroot(job: &Job) -> Result<(), container::Error> {
    let home = job.paths.build().guest;

    run(job, || {
        let mut child = process::Command::new("/bin/bash")
            .arg("--login")
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .spawn()?;

        child.wait()?;

        Ok(())
    })
}

fn run(job: &Job, f: impl FnMut() -> Result<(), container::Error>) -> Result<(), container::Error> {
    let rootfs = job.paths.rootfs().host;

    let networking = job.recipe.options.networking;

    let artefacts = job.paths.artefacts();
    let build = job.paths.build();
    let compiler = job.paths.ccache();
    let recipe = job.paths.recipe();

    Container::new(rootfs)
        .hostname("boulder")
        .networking(networking)
        .work_dir(&build.guest)
        .bind_rw(&artefacts.host, &artefacts.guest)
        .bind_rw(&build.host, &build.guest)
        .bind_rw(&compiler.host, &compiler.guest)
        .bind_ro(&recipe.host, &recipe.guest)
        .run(f)
}
