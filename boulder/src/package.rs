// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    collections::{hash_map, HashMap},
    fs, io,
};

use itertools::Itertools;
use stone::write::digest;
use stone_recipe::{script, Package};
use thiserror::Error;

use crate::{build, container, util, Macros, Paths, Recipe};

use self::collect::Collector;
use self::emit::emit;

mod analysis;
mod collect;
mod emit;

pub struct Packager {
    paths: Paths,
    recipe: Recipe,
    packages: HashMap<String, Package>,
    collector: Collector,
}

impl Packager {
    pub fn new(paths: Paths, recipe: Recipe, macros: Macros, targets: Vec<build::Target>) -> Result<Self, Error> {
        let mut collector = Collector::new(paths.install().guest);

        // Arch names used to parse [`Marcos`] for package templates
        //
        // We always use "base" plus whatever build targets we've built
        let arches = Some("base".to_string())
            .into_iter()
            .chain(targets.into_iter().map(|target| target.build_target.to_string()));

        // Resolves all package templates from arch macros + recipe file. Also adds
        // package paths to [`Collector`]
        let packages = resolve_packages(arches, &macros, &recipe, &mut collector)?;

        Ok(Self {
            paths,
            recipe,
            collector,
            packages,
        })
    }

    pub fn package(self) -> Result<(), Error> {
        // Remove old artifacts
        util::recreate_dir(&self.paths.artefacts().host).map_err(Error::RecreateArtefactsDir)?;

        // Executed in guest container since file permissions may be borked
        // for host if run rootless
        container::exec(&self.paths, false, || {
            // Hasher used for calculating file digests
            let mut hasher = digest::Hasher::new();

            // Collect all paths under install root
            let paths = self
                .collector
                .enumerate_paths(None, &mut hasher)
                .map_err(Error::CollectPaths)?;

            // Process all paths with the analysis chain
            // This will determine which files get included
            // and what deps / provides they produce
            let mut analysis = analysis::Chain::new(&self.paths, &self.recipe, &self.collector, &mut hasher);
            analysis.process(paths).map_err(Error::Analysis)?;

            // Combine the package definition with the analysis results
            // for that package. We will use this to emit the package stones & manifests.
            //
            // If no bucket exists, that means no paths matched this package so we can
            // safely filter it out
            let packages = self
                .packages
                .iter()
                .filter_map(|(name, package)| {
                    let bucket = analysis.buckets.remove(name)?;

                    Some(emit::Package::new(name, &self.recipe.parsed.source, package, bucket))
                })
                .collect::<Vec<_>>();

            // Emit package stones and manifest files to artefact directory
            emit(&self.paths, &self.recipe, &packages).map_err(Error::Emit)?;

            Ok(()) as Result<(), Error>
        })?;

        // We've exited container, sync artefacts to host
        sync_artefacts(&self.paths).map_err(Error::SyncArtefacts)?;

        Ok(())
    }
}

/// Resolve all package templates from the arch macros and
/// incoming recipe. Package templates may have variables so
/// they are fully expanded before returned.
fn resolve_packages(
    arches: impl IntoIterator<Item = String>,
    macros: &Macros,
    recipe: &Recipe,
    collector: &mut Collector,
) -> Result<HashMap<String, Package>, Error> {
    let mut parser = script::Parser::new();
    parser.add_definition("name", &recipe.parsed.source.name);
    parser.add_definition("version", &recipe.parsed.source.version);
    parser.add_definition("release", recipe.parsed.source.release);

    let mut packages = HashMap::new();

    // Add a package, ensuring it's fully expanded
    //
    // If a name collision occurs, merge the incoming and stored
    // packages
    let mut add_package = |mut name: String, mut package: Package| {
        name = parser.parse_content(&name)?;

        package.summary = package
            .summary
            .map(|summary| parser.parse_content(&summary))
            .transpose()?;
        package.description = package
            .description
            .map(|description| parser.parse_content(&description))
            .transpose()?;
        package.run_deps = package
            .run_deps
            .into_iter()
            .map(|dep| parser.parse_content(&dep))
            .collect::<Result<_, _>>()?;
        package.paths = package
            .paths
            .into_iter()
            .map(|mut path| {
                path.path = parser.parse_content(&path.path)?;
                Ok(path)
            })
            .collect::<Result<_, Error>>()?;

        // Add each path to collector
        for path in &package.paths {
            collector.add_rule(collect::Rule {
                pattern: path.path.clone(),
                package: name.clone(),
            });
        }

        match packages.entry(name.clone()) {
            hash_map::Entry::Vacant(entry) => {
                entry.insert(package);
            }
            hash_map::Entry::Occupied(entry) => {
                let prev = entry.remove();

                package.run_deps = package.run_deps.into_iter().chain(prev.run_deps).sorted().collect();
                package.paths = package
                    .paths
                    .into_iter()
                    .chain(prev.paths)
                    .sorted_by_key(|p| p.path.clone())
                    .collect();

                packages.insert(name, package);
            }
        }

        Result::<_, Error>::Ok(())
    };

    // Add packages templates from each architecture
    for arch in arches.into_iter() {
        if let Some(macros) = macros.arch.get(&arch) {
            for entry in macros.packages.clone().into_iter() {
                add_package(entry.key, entry.value)?;
            }
        }
    }

    // Add the root recipe package
    add_package(recipe.parsed.source.name.clone(), recipe.parsed.package.clone())?;

    // Add the recipe sub-packages
    recipe
        .parsed
        .sub_packages
        .iter()
        .try_for_each(|entry| add_package(entry.key.clone(), entry.value.clone()))?;

    Ok(packages)
}

fn sync_artefacts(paths: &Paths) -> Result<(), io::Error> {
    for path in util::enumerate_files(&paths.artefacts().host, |_| true)? {
        let filename = path.file_name().and_then(|p| p.to_str()).unwrap_or_default();

        let target = paths.recipe().host.join(filename);

        if target.exists() {
            fs::remove_file(&target)?;
        }

        util::hardlink_or_copy(&path, &target)?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("script")]
    Script(#[from] script::Error),
    #[error("collect install paths")]
    CollectPaths(#[source] collect::Error),
    #[error("recreate artefacts dir")]
    RecreateArtefactsDir(#[source] io::Error),
    #[error("sync artefacts")]
    SyncArtefacts(#[source] io::Error),
    #[error("analyzing paths")]
    Analysis(#[source] analysis::BoxError),
    #[error("emit packages")]
    Emit(#[from] emit::Error),
    #[error("container")]
    Container(#[from] container::Error),
}
