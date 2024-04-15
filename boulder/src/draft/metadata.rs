// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use itertools::Itertools;

use super::Upstream;

mod basic;
mod github;

#[derive(Default)]
pub struct Metadata {
    pub source: Source,
    upstreams: Vec<Upstream>,
}

#[derive(Default)]
pub struct Source {
    pub name: String,
    pub version: String,
    pub homepage: String,
}

impl Metadata {
    pub fn new(upstreams: Vec<Upstream>) -> Self {
        let mut source = Source::default();

        // Try to identify source metadata from the first upstream
        if let Some(upstream) = upstreams.first() {
            for matcher in Matcher::ALL {
                if let Some(matched) = match matcher {
                    Matcher::Basic => basic::source(&upstream.uri),
                    Matcher::Github => github::source(&upstream.uri),
                } {
                    source = matched;
                    break;
                }
            }
        }

        Self { source, upstreams }
    }

    pub fn upstreams(&self) -> String {
        self.upstreams
            .iter()
            .map(|Upstream { uri, hash }| format!("    - {uri} : {hash}"))
            .join("\n")
    }
}

enum Matcher {
    Basic,
    Github,
}

impl Matcher {
    const ALL: &'static [Self] = &[Self::Basic, Self::Github];
}
