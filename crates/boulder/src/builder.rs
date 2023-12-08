// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fs, io, os::unix::process::ExitStatusExt, path::Path, process};

use nix::{sys::signal::Signal, unistd::Pid};
use stone_recipe::Recipe;
use thiserror::Error;
use tui::Stylize;

use crate::{
    architecture::BuildTarget,
    container::{self, ExecError},
    job::{self, Step},
    macros, paths, pgo, profile, recipe, root, upstream, Env, Job, Macros, Paths, Runtime,
};

pub struct Builder {
    pub targets: Vec<Target>,
    pub recipe: Recipe,
    pub paths: Paths,
    pub macros: Macros,
    pub ccache: bool,
    pub env: Env,
    profile: profile::Id,
}

pub struct Target {
    pub build_target: BuildTarget,
    pub jobs: Vec<Job>,
}

impl Builder {
    pub fn new(
        recipe_path: &Path,
        env: Env,
        profile: profile::Id,
        ccache: bool,
    ) -> Result<Self, Error> {
        let recipe_bytes = fs::read(recipe_path)?;
        let recipe = stone_recipe::from_slice(&recipe_bytes)?;

        let macros = Macros::load(&env)?;

        let paths = Paths::new(
            paths::Id::new(&recipe),
            recipe_path,
            &env.cache_dir,
            "/mason",
        )?;

        let build_targets = recipe::build_targets(&recipe);

        if build_targets.is_empty() {
            return Err(Error::NoBuildTargets);
        }

        let targets = build_targets
            .into_iter()
            .map(|build_target| {
                let stages = pgo::stages(&recipe, build_target)
                    .map(|stages| stages.into_iter().map(Some).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![None]);

                let jobs = stages
                    .into_iter()
                    .map(|stage| Job::new(build_target, stage, &recipe, &paths, &macros, ccache))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Target { build_target, jobs })
            })
            .collect::<Result<Vec<_>, job::Error>>()?;

        Ok(Self {
            targets,
            recipe,
            paths,
            macros,
            ccache,
            env,
            profile,
        })
    }

    pub fn extra_deps(&self) -> impl Iterator<Item = &str> {
        self.targets.iter().flat_map(|target| {
            target.jobs.iter().flat_map(|job| {
                job.steps
                    .values()
                    .flat_map(|script| script.dependencies.iter().map(String::as_str))
            })
        })
    }

    pub fn setup(&self) -> Result<(), Error> {
        root::clean(self)?;

        let rt = Runtime::new()?;

        rt.block_on(async {
            let profiles = profile::Manager::new(&self.env).await;

            let repos = profiles.repositories(&self.profile)?.clone();

            root::populate(self, repos).await?;
            upstream::sync(&self.recipe, &self.paths).await?;

            Ok(()) as Result<_, Error>
        })?;

        Ok(())
    }

    pub fn build(self) -> Result<(), Error> {
        container::exec(&self.paths, self.recipe.options.networking, || {
            // We're now in the container =)

            for (i, target) in self.targets.iter().enumerate() {
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
                        println!("{}", format!("│pgo-{stage}").dim());
                    }

                    for (i, (step, script)) in job.steps.iter().enumerate() {
                        let pipes = if job.pgo_stage.is_some() {
                            "││".dim()
                        } else {
                            "│".dim()
                        };

                        if i > 0 {
                            println!("{pipes}");
                        }
                        println!("{pipes}{}", step.styled(format!("{step}")));

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

                        ::container::forward_sigint(Pid::from_raw(command.id() as i32))?;

                        let result = command.wait()?;

                        if !result.success() {
                            match result.code() {
                                Some(code) => {
                                    return Err(ExecError::Code(code));
                                }
                                None => {
                                    if let Some(signal) = result
                                        .signal()
                                        .or_else(|| result.stopped_signal())
                                        .and_then(|i| Signal::try_from(i).ok())
                                    {
                                        return Err(ExecError::Signal(signal));
                                    } else {
                                        return Err(ExecError::UnknownSignal);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        })?;
        Ok(())
    }
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
fn log(step: Step, is_pgo: bool) -> Result<process::Child, io::Error> {
    let pgo = is_pgo.then_some("│").unwrap_or_default().dim();
    let kind = step.styled(format!("{}│", step.abbrev()));
    let tag = format!("{}{pgo}{kind} ", "│".dim());

    process::Command::new("awk")
        .arg(format!(r#"{{ print "{tag}" $0 }}"#))
        .env("PATH", "/usr/bin:/usr/sbin")
        .env("TERM", "xterm-256color")
        .stdin(process::Stdio::piped())
        .spawn()
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("no supported build targets for recipe")]
    NoBuildTargets,
    #[error("macros")]
    Macros(#[from] macros::Error),
    #[error("job")]
    Job(#[from] job::Error),
    #[error("profile")]
    Profile(#[from] profile::Error),
    #[error("root")]
    Root(#[from] root::Error),
    #[error("upstream")]
    Upstream(#[from] upstream::Error),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("container")]
    Container(#[from] container::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
