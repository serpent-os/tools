// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    collections::{HashMap, HashSet},
    fmt,
    num::NonZeroU64,
    path::{Path, PathBuf},
};

use moss::Dependency;

mod cargo;
mod cmake;

pub type Error = Box<dyn std::error::Error>;

/// A build system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum System {
    Cargo,
    Cmake,
}

impl System {
    const ALL: &'static [Self] = &[Self::Cargo, Self::Cmake];

    pub fn phases(&self) -> Phases {
        match self {
            System::Cargo => cargo::phases(),
            System::Cmake => cmake::phases(),
        }
    }

    fn process(&self, state: &mut State, path: &Path) -> Result<(), Error> {
        match self {
            System::Cargo => cargo::process(state, path),
            System::Cmake => cmake::process(state, path),
        }
    }
}

/// Commands to run for each build phase of the [`System`]
pub struct Phases {
    pub environment: Option<&'static str>,
    pub setup: Option<&'static str>,
    pub build: Option<&'static str>,
    pub install: Option<&'static str>,
    pub check: Option<&'static str>,
}

impl fmt::Display for Phases {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut fmt = |name, value| {
            if let Some(value) = value {
                writeln!(f, "{name:<12}: |\n    {value}")
            } else {
                Ok(())
            }
        };
        fmt("environment", self.environment)?;
        fmt("setup", self.setup)?;
        fmt("build", self.build)?;
        fmt("install", self.install)?;
        fmt("check", self.check)
    }
}

/// State passed to each system when processing paths
struct State<'a> {
    /// Any dependencies that need to be recorded
    dependencies: &'a mut HashSet<Dependency>,
    /// Total confidence level of the current build [`System`]
    confidence: u64,
}

impl<'a> State<'a> {
    /// Increase the confidence that this project uses the current build [`System`]
    pub fn increment_confidence(&mut self, amount: u64) {
        self.confidence += amount;
    }

    /// Add a dependency to output in `builddeps`
    pub fn add_dependency(&mut self, dependency: Dependency) {
        self.dependencies.insert(dependency);
    }
}

/// Analysis results from [`analyze`]
pub struct Analysis {
    /// The detected build [`System`], if any
    pub detected_system: Option<System>,
    /// All detected dependencies
    pub dependencies: HashSet<Dependency>,
}

/// Analyze the provided paths to determine which build [`System`]
/// the project uses and any dependencies that are identified
pub fn analyze(paths: &[PathBuf]) -> Result<Analysis, Error> {
    let mut dependencies = HashSet::new();
    let mut confidences = HashMap::new();

    for system in System::ALL {
        let mut state = State {
            dependencies: &mut dependencies,
            confidence: 0,
        };

        for path in paths {
            system.process(&mut state, path)?;
        }

        if let Some(confidence) = NonZeroU64::new(state.confidence) {
            confidences.insert(*system, confidence);
        }
    }

    let detected_system = confidences
        .into_iter()
        .max_by_key(|(_, confidence)| *confidence)
        .map(|(system, _)| system);

    Ok(Analysis {
        detected_system,
        dependencies,
    })
}
