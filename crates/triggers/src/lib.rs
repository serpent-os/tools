// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! System trigger management facilities

use format::Trigger;
use thiserror::Error;

pub mod format;

pub struct Manager {
    handlers: Vec<ExtractedHandler>,
}

#[derive(Debug)]
struct ExtractedHandler {
    trigger: String,
    handler: format::Handler,
    pattern: fnmatch::Pattern,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing handler reference in {0}: {1}")]
    MissingHandler(String, String),
}

impl Manager {
    /// Create a new [Manager] using the given triggers
    pub fn new(triggers: Vec<Trigger>) -> Result<Self, Error> {
        let mut handlers = vec![];
        for trigger in triggers.iter() {
            for (p, def) in trigger.paths.iter() {
                for used_handler in def.handlers.iter() {
                    let found = trigger
                        .handlers
                        .get(used_handler)
                        .ok_or(Error::MissingHandler(
                            trigger.name.clone(),
                            used_handler.clone(),
                        ))?;
                    handlers.push(ExtractedHandler {
                        trigger: trigger.name.clone(),
                        handler: found.clone(),
                        pattern: p.clone(),
                    });
                }
            }
        }

        Ok(Self { handlers })
    }

    /// Push a path, building up our matches
    pub fn push_path(&mut self, path: &str) {
        for (h, m) in self
            .handlers
            .iter()
            .filter_map(|h| h.pattern.match_path(path).map(|m| (h, m)))
        {
            eprintln!("Matching [{}]: {:?} : {:?}", h.trigger, m, h.handler);
        }
    }
}
