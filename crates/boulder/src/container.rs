// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io,
    process::{self, Child, Stdio},
};

use container::Container;
use stone_recipe::Script;
use tui::Stylize;

use crate::{job::Step, Job};

pub fn chroot(job: &Job) -> Result<(), container::Error> {
    // TODO: Archify
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

// TODO: Arch / profiles
pub fn exec(step: Step, job: &Job, script: &Script) -> Result<(), container::Error> {
    // TODO: Archify
    let home = job.paths.build().guest;
    let work_dir = job.work_dir();

    run(job, || {
        // We're in the container now =)
        // TODO: Proper temp file
        let script_path = "/tmp/script";
        std::fs::write(script_path, &script.content).unwrap();

        let current_dir = if work_dir.exists() { &work_dir } else { &home };

        let mut command = logged(step, "/bin/sh")?
            .arg(script_path)
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .current_dir(current_dir)
            .spawn()?;

        command.wait()?;

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

fn logged(step: Step, command: &str) -> Result<process::Command, io::Error> {
    let out_log = log(step)?;
    let err_log = log(step)?;

    let mut command = process::Command::new(command);
    command
        .stdout(out_log.stdin.unwrap())
        .stderr(err_log.stdin.unwrap());

    Ok(command)
}

fn log(step: Step) -> Result<Child, io::Error> {
    let step = step.styled(format!("{step:>8}"));
    let tag = format!("{step} {} ", ":".dim());

    process::Command::new("awk")
        .arg(format!(r#"{{ print "{tag}" $0 }}"#))
        .env("PATH", "/usr/bin:/usr/sbin")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .spawn()
}
