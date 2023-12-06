// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io,
    process::{self, Child, Stdio},
};

use container::Container;
pub use container::Error;
use stone_recipe::Script;
use tui::Stylize;

use crate::{job::Step, Job, Paths};

pub fn chroot(paths: &Paths, networking: bool) -> Result<(), Error> {
    let home = &paths.build().guest;

    run(paths, networking, || {
        let mut child = process::Command::new("/bin/bash")
            .arg("--login")
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .spawn()?;

        child.wait()?;

        Ok(())
    })
}

pub fn exec(step: Step, paths: &Paths, job: &Job, script: &Script) -> Result<(), Error> {
    let build_dir = &job.build_dir;
    let work_dir = &job.work_dir;
    let emul32 = job.target.emul32();

    run(paths, job.networking, || {
        // We're in the container now =)
        // TODO: Proper temp file
        let script_path = "/tmp/script";
        std::fs::write(script_path, &script.content).unwrap();

        let current_dir = if work_dir.exists() {
            &work_dir
        } else {
            &build_dir
        };

        let mut command = logged(step, emul32, "/bin/sh")?
            .arg(script_path)
            .env_clear()
            .env("HOME", build_dir)
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .current_dir(current_dir)
            .spawn()?;

        command.wait()?;

        Ok(())
    })
}

fn run(paths: &Paths, networking: bool, f: impl FnMut() -> Result<(), Error>) -> Result<(), Error> {
    let rootfs = paths.rootfs().host;
    let artefacts = paths.artefacts();
    let build = paths.build();
    let compiler = paths.ccache();
    let recipe = paths.recipe();

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

fn logged(step: Step, emul32: bool, command: &str) -> Result<process::Command, io::Error> {
    let out_log = log(step, emul32)?;
    let err_log = log(step, emul32)?;

    let mut command = process::Command::new(command);
    command
        .stdout(out_log.stdin.unwrap())
        .stderr(err_log.stdin.unwrap());

    Ok(command)
}

// TODO: Ikey plz make look nice
fn log(step: Step, emul32: bool) -> Result<Child, io::Error> {
    let min_fill = if step.pgo_stage.is_some() { 13 } else { 7 };

    let fill_len = min_fill
        - step.kind.to_string().len()
        - step
            .pgo_stage
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default()
            .len();
    let fill = format!("{:>fill_len$}", "");

    let emul32 = emul32.then_some("(emul32) ").unwrap_or_default().red();
    let pgo = step
        .pgo_stage
        .map(|stage| format!("({stage}) "))
        .unwrap_or_default()
        .dim();
    let kind = step.kind.styled(step.kind);
    let tag = format!("{fill}{emul32}{pgo}{kind} {} ", ":".dim());

    process::Command::new("awk")
        .arg(format!(r#"{{ print "{tag}" $0 }}"#))
        .env("PATH", "/usr/bin:/usr/sbin")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .spawn()
}
