// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

use stone_recipe::{script, tuning::Toolchain, Recipe, Script, Upstream};
use thiserror::Error;
use tui::Stylize;

pub use self::pgo::Pgo;
use crate::{architecture::BuildTarget, util, Macros, Paths};

mod pgo;

#[derive(Debug)]
pub struct Job {
    pub target: BuildTarget,
    pub scripts: BTreeMap<Step, Script>,
    pub work_dir: PathBuf,
    pub build_dir: PathBuf,
    pub networking: bool,
}

impl Job {
    pub async fn new(
        target: BuildTarget,
        recipe: &Recipe,
        paths: &Paths,
        macros: &Macros,
        ccache: bool,
    ) -> Result<Self, Error> {
        let build_dir = paths.build().guest.join(target.to_string());
        let work_dir = work_dir(&build_dir, &recipe.upstreams);
        let networking = recipe.options.networking;

        let pgo = Pgo::new(target, recipe, &build_dir);

        let scripts = Step::ALL
            .iter()
            .filter_map(|step| {
                let result = step
                    .script(target, recipe, paths, macros, ccache)
                    .transpose()?;
                Some(result.map(|script| (*step, script)))
            })
            .collect::<Result<_, _>>()?;

        // Clean build dir & pgo from host (we're not in container yet)
        let host_build_dir = paths.build().host.join(target.to_string());
        util::recreate_dir(&host_build_dir).await?;

        if pgo.is_some() {
            let host_pgo_dir = PathBuf::from(format!("{}-pgo", host_build_dir.display()));
            util::recreate_dir(&host_pgo_dir).await?;
        }

        Ok(Self {
            target,
            scripts,
            work_dir,
            build_dir,
            networking,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum Step {
    Prepare,
    Setup,
    Build,
    Check,
    Install,
    Workload,
}

impl Step {
    const ALL: &'static [Self] = &[
        Self::Prepare,
        Self::Setup,
        Self::Build,
        Self::Check,
        Self::Install,
        Self::Workload,
    ];

    pub fn styled(&self, s: impl ToString) -> String {
        let s = s.to_string();
        // Taste the rainbow
        // TODO: Ikey plz make pretty
        match self {
            Step::Prepare => s.grey(),
            Step::Setup => s.cyan(),
            Step::Build => s.blue(),
            Step::Check => s.yellow(),
            Step::Install => s.green(),
            Step::Workload => s.magenta(),
        }
        .dim()
        .to_string()
    }

    fn script(
        &self,
        target: BuildTarget,
        recipe: &Recipe,
        paths: &Paths,
        macros: &Macros,
        ccache: bool,
    ) -> Result<Option<Script>, Error> {
        let Some(mut content) = (match self {
            Step::Prepare => Some(prepare_script(recipe)),
            Step::Setup => recipe.build.setup.clone(),
            Step::Build => recipe.build.build.clone(),
            Step::Check => recipe.build.check.clone(),
            Step::Install => recipe.build.install.clone(),
            Step::Workload => recipe.build.workload.clone(),
        }) else {
            return Ok(None);
        };

        if content.is_empty() {
            return Ok(None);
        }

        if let Some(env) = recipe.build.environment.as_deref() {
            if env != "(null)" && !env.is_empty() && !matches!(self, Step::Prepare) {
                content = format!("{env} {content}");
            }
        }

        content = format!("%scriptBase\n{content}");

        let mut parser = script::Parser::new();

        let build_target = target.to_string();
        let build_dir = paths.build().guest.join(&build_target);
        let work_dir = if matches!(self, Step::Prepare) {
            build_dir.clone()
        } else {
            work_dir(&build_dir, &recipe.upstreams)
        };
        let num_jobs = util::num_cpus();

        for arch in ["base", &build_target] {
            let macros = macros
                .arch
                .get(arch)
                .cloned()
                .ok_or_else(|| Error::MissingArchMacros(arch.to_string()))?;

            parser.add_macros(macros);
        }

        for macros in macros.actions.clone() {
            parser.add_macros(macros);
        }

        parser.add_definition("name", &recipe.source.name);
        parser.add_definition("version", &recipe.source.version);
        parser.add_definition("release", recipe.source.release);
        parser.add_definition("jobs", num_jobs);
        parser.add_definition("pkgdir", paths.recipe().guest.join("pkg").display());
        parser.add_definition("sourcedir", paths.upstreams().guest.display());
        parser.add_definition("installroot", paths.install().guest.display());
        parser.add_definition("buildroot", build_dir.display());
        parser.add_definition("workdir", work_dir.display());

        // TODO: Remaining definitions & tune flags
        parser.add_definition("cflags", "");
        parser.add_definition("cxxflags", "");
        parser.add_definition("ldflags", "");

        parser.add_definition("compiler_cache", "/mason/ccache");

        let path = if ccache {
            "/usr/lib/ccache/bin:/usr/bin:/bin"
        } else {
            "/usr/bin:/bin"
        };

        /* Set the relevant compilers */
        if matches!(recipe.options.toolchain, Toolchain::Llvm) {
            parser.add_definition("compiler_c", "clang");
            parser.add_definition("compiler_cxx", "clang++");
            parser.add_definition("compiler_objc", "clang");
            parser.add_definition("compiler_objcxx", "clang++");
            parser.add_definition("compiler_cpp", "clang -E -");
            parser.add_definition("compiler_objcpp", "clang -E -");
            parser.add_definition("compiler_objcxxcpp", "clang++ -E");
            parser.add_definition("compiler_ar", "llvm-ar");
            parser.add_definition("compiler_ld", "ld.lld");
            parser.add_definition("compiler_objcopy", "llvm-objcopy");
            parser.add_definition("compiler_nm", "llvm-nm");
            parser.add_definition("compiler_ranlib", "llvm-ranlib");
            parser.add_definition("compiler_strip", "llvm-strip");
            parser.add_definition("compiler_path", path);
        } else {
            parser.add_definition("compiler_c", "gcc");
            parser.add_definition("compiler_cxx", "g++");
            parser.add_definition("compiler_objc", "gcc");
            parser.add_definition("compiler_objcxx", "g++");
            parser.add_definition("compiler_cpp", "gcc -E");
            parser.add_definition("compiler_objcpp", "gcc -E");
            parser.add_definition("compiler_objcxxcpp", "g++ -E");
            parser.add_definition("compiler_ar", "gcc-ar");
            parser.add_definition("compiler_ld", "ld.bfd");
            parser.add_definition("compiler_objcopy", "objcopy");
            parser.add_definition("compiler_nm", "gcc-nm");
            parser.add_definition("compiler_ranlib", "gcc-ranlib");
            parser.add_definition("compiler_strip", "strip");
            parser.add_definition("compiler_path", path);
        }

        parser.add_definition("pgo_dir", format!("{}-pgo", build_dir.display()));

        Ok(Some(parser.parse(&content)?))
    }
}

fn prepare_script(recipe: &Recipe) -> String {
    use std::fmt::Write;

    let mut content = String::default();

    for upstream in &recipe.upstreams {
        match upstream {
            stone_recipe::Upstream::Plain {
                uri,
                rename,
                strip_dirs,
                unpack,
                unpack_dir,
                ..
            } => {
                if !*unpack {
                    continue;
                }
                let file_name = util::uri_file_name(uri);
                let rename = rename.as_deref().unwrap_or(file_name);
                let unpack_dir = unpack_dir
                    .as_ref()
                    .map(|dir| dir.display().to_string())
                    .unwrap_or_else(|| rename.to_string());
                let strip_dirs = strip_dirs.unwrap_or(1);

                let _ = writeln!(&mut content, "mkdir -p {unpack_dir}");
                if rename.ends_with(".zip") {
                    let _ = writeln!(
                        &mut content,
                        r#"unzip -d "{unpack_dir}" "%(sourcedir)/{rename}" || (echo "Failed to extract arcive"; exit 1);"#,
                    );
                } else {
                    let _ = writeln!(
                        &mut content,
                        r#"tar xf "%(sourcedir)/{rename}" -C "{unpack_dir}" --strip-components={strip_dirs} --no-same-owner || (echo "Failed to extract arcive"; exit 1);"#,
                    );
                }
            }
            stone_recipe::Upstream::Git { uri, clone_dir, .. } => {
                let source = util::uri_file_name(uri);
                let target = clone_dir
                    .as_ref()
                    .map(|dir| dir.display().to_string())
                    .unwrap_or_else(|| source.to_string());

                let _ = writeln!(&mut content, "mkdir -p {target}");
                let _ = writeln!(
                    &mut content,
                    r#"cp -Ra --no-preserve=ownership "%(sourcedir)/{source}/." "{target}""#,
                );
            }
        }
    }

    content
}

fn work_dir(build_dir: &Path, upstreams: &[Upstream]) -> PathBuf {
    let mut work_dir = build_dir.to_path_buf();

    // Work dir is the first upstream that should be unpacked
    if let Some(upstream) = upstreams.iter().find(|upstream| match upstream {
        Upstream::Plain { unpack, .. } => *unpack,
        Upstream::Git { .. } => true,
    }) {
        match upstream {
            Upstream::Plain {
                uri,
                rename,
                unpack_dir,
                ..
            } => {
                let file_name = util::uri_file_name(uri);
                let rename = rename.as_deref().unwrap_or(file_name);
                let unpack_dir = unpack_dir
                    .as_ref()
                    .map(|dir| dir.display().to_string())
                    .unwrap_or_else(|| rename.to_string());

                work_dir = build_dir.join(unpack_dir);
            }
            Upstream::Git { uri, clone_dir, .. } => {
                let source = util::uri_file_name(uri);
                let target = clone_dir
                    .as_ref()
                    .map(|dir| dir.display().to_string())
                    .unwrap_or_else(|| source.to_string());

                work_dir = build_dir.join(target);
            }
        }
    }

    work_dir
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing arch macros: {0}")]
    MissingArchMacros(String),
    #[error("script")]
    Script(#[from] script::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
