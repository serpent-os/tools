// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! System trigger management facilities

use std::collections::BTreeMap;

use format::Trigger;
use thiserror::Error;

pub mod format;

/// Grouped management of a set of triggers
pub struct Collection<'a> {
    handlers: Vec<ExtractedHandler>,
    triggers: BTreeMap<String, &'a Trigger>,
    hits: Vec<(String, fnmatch::Match, format::Handler)>,
}

// Return type: Map a handler to a set of matching files.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TriggerCommand {
    pub handler: format::Handler,
    files: Vec<String>,
}

#[derive(Debug)]
struct ExtractedHandler {
    id: String,
    pattern: fnmatch::Pattern,
    handler: format::Handler,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing handler reference in {0}: {1}")]
    MissingHandler(String, String),
}

impl<'a> Collection<'a> {
    /// Create a new [Collection] using the given triggers
    pub fn new(triggers: impl IntoIterator<Item = &'a Trigger>) -> Result<Self, Error> {
        let mut handlers = vec![];
        let mut trigger_set = BTreeMap::new();
        for trigger in triggers.into_iter() {
            trigger_set.insert(trigger.name.clone(), trigger);
            for (p, def) in trigger.paths.iter() {
                for used_handler in def.handlers.iter() {
                    // Ensure we have a corresponding handler
                    let handler = trigger
                        .handlers
                        .get(used_handler)
                        .ok_or(Error::MissingHandler(trigger.name.clone(), used_handler.clone()))?;
                    handlers.push(ExtractedHandler {
                        id: trigger.name.clone(),
                        pattern: p.clone(),
                        handler: handler.clone(),
                    });
                }
            }
        }

        Ok(Self {
            handlers,
            triggers: trigger_set,
            hits: vec![],
        })
    }

    /// Process a batch set of paths and record the "hit"
    pub fn process_paths(&mut self, paths: impl Iterator<Item = String>) {
        let results = paths.into_iter().flat_map(|p| {
            self.handlers
                .iter()
                .filter_map(move |h| h.pattern.match_path(&p).map(|m| (h.id.clone(), m, h.handler.clone())))
        });
        self.hits.extend(results);
    }

    /// Bake the trigger collection into a sane dependency order
    pub fn bake(&self) -> Result<Vec<TriggerCommand>, Error> {
        let mut graph: dag::Dag<String> = dag::Dag::new();

        // OK, now lets order by deps..
        for (id, _, _) in self.hits.iter() {
            let lookup = self
                .triggers
                .get(id)
                .ok_or(Error::MissingHandler(id.clone(), id.clone()))?;

            let node = graph.add_node_or_get_index(id.clone());

            // This runs *before* B
            if let Some(before) = lookup
                .before
                .as_ref()
                .and_then(|b| self.triggers.get(b))
                .map(|f| graph.add_node_or_get_index(f.name.clone()))
            {
                graph.add_edge(before, node);
            }

            // This runs *after* A
            if let Some(after) = lookup
                .after
                .as_ref()
                .and_then(|a| self.triggers.get(a))
                .map(|f| graph.add_node_or_get_index(f.name.clone()))
            {
                graph.add_edge(node, after);
            }
        }

        // Recollect in dependency order
        let built_triggers = graph
            .topo()
            .flat_map(|i| self.hits.iter().filter(move |(id, _, _)| i == id));

        let mut runnables: BTreeMap<format::Handler, TriggerCommand> = BTreeMap::new();
        for (_, hit, handler) in built_triggers {
            let handler = handler.substituted(hit);

            if let Some(store) = runnables.get_mut(&handler) {
                store.files.push(hit.path.clone());
            } else {
                runnables.insert(
                    handler.clone(),
                    TriggerCommand {
                        handler,
                        files: vec![hit.path.clone()],
                    },
                );
            }
        }

        Ok(runnables.values().cloned().collect::<Vec<_>>())
    }
}
