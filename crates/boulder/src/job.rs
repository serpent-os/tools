// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use stone_recipe::{script, tuning::Toolchain, Recipe, Script, Upstream};
use thiserror::Error;
use tui::Stylize;

use crate::{macros, util, Env, Macros};

#[derive(Debug, Clone)]
pub struct Id(String);

impl Id {
    fn new(recipe: &Recipe) -> Self {
        Self(format!(
            "{}-{}-{}",
            recipe.source.name, recipe.source.version, recipe.source.release
        ))
    }
}

#[derive(Debug)]
pub struct Job {
    pub id: Id,
    pub recipe: Recipe,
    pub paths: Paths,
    pub macros: Macros,
    pub scripts: BTreeMap<Step, Script>,
}

impl Job {
    pub async fn new(recipe_path: &Path, env: &Env) -> Result<Self, Error> {
        let recipe_bytes = fs::read(recipe_path)?;
        let recipe = stone_recipe::from_slice(&recipe_bytes)?;

        let id = Id::new(&recipe);

        let paths = Paths::new(id.clone(), recipe_path, &env.cache_dir, "/mason").await?;
        let macros = Macros::load(env).await?;

        let scripts = Step::ALL
            .iter()
            .filter_map(|step| {
                let result = step.script(&recipe, &paths, &macros).transpose()?;
                Some(result.map(|script| (*step, script)))
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            id,
            recipe,
            paths,
            macros,
            scripts,
        })
    }

    pub fn work_dir(&self) -> PathBuf {
        work_dir(&self.paths, &self.recipe.upstreams)
    }
}

#[derive(Debug)]
pub struct Paths {
    id: Id,
    host_root: PathBuf,
    guest_root: PathBuf,
    recipe_dir: PathBuf,
}

impl Paths {
    async fn new(
        id: Id,
        recipe_path: &Path,
        host_root: impl Into<PathBuf>,
        guest_root: impl Into<PathBuf>,
    ) -> io::Result<Self> {
        let recipe_dir = recipe_path
            .parent()
            .unwrap_or(&PathBuf::default())
            .canonicalize()?;

        let job = Self {
            id,
            host_root: host_root.into().canonicalize()?,
            guest_root: guest_root.into(),
            recipe_dir,
        };

        util::ensure_dir_exists(&job.rootfs().host).await?;
        util::ensure_dir_exists(&job.artefacts().host).await?;
        util::ensure_dir_exists(&job.build().host).await?;
        util::ensure_dir_exists(&job.ccache().host).await?;
        util::ensure_dir_exists(&job.upstreams().host).await?;

        Ok(job)
    }

    pub fn rootfs(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("root").join(&self.id.0),
            guest: "/".into(),
        }
    }

    pub fn artefacts(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("artefacts").join(&self.id.0),
            guest: self.guest_root.join("artefacts"),
        }
    }

    pub fn build(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("build").join(&self.id.0),
            guest: self.guest_root.join("build"),
        }
    }

    pub fn ccache(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("ccache"),
            guest: self.guest_root.join("ccache"),
        }
    }

    pub fn upstreams(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("upstreams"),
            guest: self.guest_root.join("sourcedir"),
        }
    }

    pub fn recipe(&self) -> PathMapping {
        PathMapping {
            host: self.recipe_dir.clone(),
            guest: self.guest_root.join("recipe"),
        }
    }

    pub fn install(&self) -> PathMapping {
        PathMapping {
            // TODO: Shitty impossible state, this folder
            // doesn't exist on host
            host: "".into(),
            guest: self.guest_root.join("install"),
        }
    }

    /// For the provided [`Mapping`], return the guest
    /// path as it lives on the host fs
    ///
    /// Example:
    /// - host = "/var/cache/boulder/root/test"
    /// - guest = "/mason/build"
    /// - guest_host_path = "/var/cache/boulder/root/test/mason/build"
    pub fn guest_host_path(&self, mapping: &PathMapping) -> PathBuf {
        let relative = mapping.guest.strip_prefix("/").unwrap_or(&mapping.guest);

        self.rootfs().host.join(relative)
    }
}

pub struct PathMapping {
    pub host: PathBuf,
    pub guest: PathBuf,
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
        recipe: &Recipe,
        paths: &Paths,
        macros: &Macros,
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

        let work_dir = if matches!(self, Step::Prepare) {
            paths.build().guest.clone()
        } else {
            work_dir(paths, &recipe.upstreams)
        };
        let num_jobs = util::num_cpus();

        // TODO: Handle actual arch
        for arch in ["base", "x86_64"] {
            let macros = macros
                .arch
                .get(arch)
                .cloned()
                .ok_or_else(|| Error::MissingArchMacros(arch.to_string()))?;

            parser.add_macros(macros);

            // TODO: Add arch
            parser.add_definition("buildroot", paths.build().guest.display());
            parser.add_definition("workdir", work_dir.display());
        }

        for macros in macros.actions.clone() {
            parser.add_macros(macros);
        }

        parser.add_definition("name", &recipe.source.name);
        parser.add_definition("version", &recipe.source.version);
        parser.add_definition("release", recipe.source.release);
        // TODO: Jobs
        parser.add_definition("jobs", num_jobs);
        parser.add_definition("pkgdir", paths.recipe().guest.join("pkg").display());
        parser.add_definition("sourcedir", paths.upstreams().guest.display());
        parser.add_definition("installroot", paths.install().guest.display());

        // TODO: Remaining definitions & tune flags
        parser.add_definition("cflags", "");
        parser.add_definition("cxxflags", "");
        parser.add_definition("ldflags", "");

        parser.add_definition("compiler_cache", "/mason/ccache");

        let path = "/usr/bin:/bin";

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
            stone_recipe::Upstream::Git { .. } => todo!(),
        }
    }

    content
}

fn work_dir(paths: &Paths, upstreams: &[Upstream]) -> PathBuf {
    let build_dir = paths.build().guest;
    let mut work_dir = build_dir.clone();

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
            Upstream::Git { .. } => todo!(),
        }
    }

    work_dir
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing arch macros: {0}")]
    MissingArchMacros(String),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("macros")]
    Macros(#[from] macros::Error),
    #[error("script")]
    Script(#[from] script::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
