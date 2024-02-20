use std::collections::{hash_map, HashMap};

use itertools::Itertools;
use stone_recipe::{script, Package};
use thiserror::Error;

use crate::{build, Macros, Recipe};

pub use self::matcher::Matcher;

pub mod matcher;

pub struct Packager {
    packages: HashMap<String, Package>,
    matcher: Matcher,
    recipe: Recipe,
}

impl Packager {
    pub fn new(recipe: Recipe, macros: Macros, targets: Vec<build::Target>) -> Result<Self, Error> {
        // Arch names used to parse [`Marcos`] for package templates
        //
        // We always use "base" plus whatever build targets we've built
        let arches = Some("base".to_string())
            .into_iter()
            .chain(targets.into_iter().map(|target| target.build_target.to_string()));

        // Resolves all package templates from arch macros + recipe file
        let packages = resolve_packages(arches, &macros, &recipe)?;

        let mut matcher = Matcher::default();

        // Add all package files to the matcher
        for (name, package) in &packages {
            for path in &package.paths {
                matcher.add_rule(matcher::Rule {
                    pattern: path.path.clone(),
                    target: name.clone(),
                });
            }
        }

        Ok(Self {
            matcher,
            packages,
            recipe,
        })
    }

    pub fn package(self) -> Result<(), Error> {
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("script")]
    Script(#[from] script::Error),
}
