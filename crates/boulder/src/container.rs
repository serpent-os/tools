// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io,
    process::{self, Child, Stdio},
};

use container::Container;
pub use container::Error;
use tui::Stylize;

use crate::{job::Step, Builder, Paths};

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

pub fn build(builder: Builder) -> Result<(), Error> {
    let paths = &builder.paths;
    let networking = builder.recipe.options.networking;

    run(paths, networking, || {
        // We're now in the container =)

        for (i, target) in builder.targets.iter().enumerate() {
            if i > 0 {
                println!();
            }
            println!("{}", target.build_target.to_string().dim());

            for (i, job) in target.jobs.iter().enumerate() {
                let is_pgo = job.pgo_stage.is_some();

                if let Some(stage) = job.pgo_stage {
                    if i > 0 {
                        println!("{}", "│".dim());
                    }
                    println!("{}", format!("│ pgo-{stage}").dim());
                }

                for (step, script) in job.steps.iter() {
                    let build_dir = &job.build_dir;
                    let work_dir = &job.work_dir;

                    // TODO: Proper temp file
                    let script_path = "/tmp/script";
                    std::fs::write(script_path, &script.content).unwrap();

                    let current_dir = if work_dir.exists() {
                        &work_dir
                    } else {
                        &build_dir
                    };

                    let mut command = logged(*step, is_pgo, "/bin/sh")?
                        .arg(script_path)
                        .env_clear()
                        .env("HOME", build_dir)
                        .env("PATH", "/usr/bin:/usr/sbin")
                        .env("TERM", "xterm-256color")
                        .current_dir(current_dir)
                        .spawn()?;

                    command.wait()?;
                }
            }
        }

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

fn logged(step: Step, is_pgo: bool, command: &str) -> Result<process::Command, io::Error> {
    let out_log = log(step, is_pgo)?;
    let err_log = log(step, is_pgo)?;

    let mut command = process::Command::new(command);
    command
        .stdout(out_log.stdin.unwrap())
        .stderr(err_log.stdin.unwrap());

    Ok(command)
}

// TODO: Ikey plz make look nice
fn log(step: Step, is_pgo: bool) -> Result<Child, io::Error> {
    let pgo = is_pgo.then_some("│ ").unwrap_or_default().dim();
    let kind = step.styled(format!("{step:>7}"));
    let tag = format!("{} {pgo}{kind} {} ", "│".dim(), ":".dim());

    process::Command::new("awk")
        .arg(format!(r#"{{ print "{tag}" $0 }}"#))
        .env("PATH", "/usr/bin:/usr/sbin")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .spawn()
}
