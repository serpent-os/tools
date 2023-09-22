// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Pretty printing for moss CLI

use std::cmp::max;

use itertools::Itertools;
use moss::Package;
use tui::{term_size, Stylize};

/// Print packages as column output
pub fn print_to_columns<T>(items: T)
where
    T: IntoIterator<Item = Package>,
{
    let terminal_width = term_size().width;

    // Map into something simple
    let mut mapped = items
        .into_iter()
        .map(|p| State {
            name: p.meta.name.to_string(),
            version: format!("{}-{}", p.meta.version_identifier, p.meta.source_release),
        })
        .collect_vec();
    mapped.sort();

    // Figure render constraints
    let largest_element = mapped
        .iter()
        .max_by_key(|p| p.name.len() + p.version.len() + 3)
        .unwrap();
    let largest_width = largest_element.name.len() + largest_element.version.len() + 6;
    let num_columns = max(1, terminal_width / largest_width);
    let height = ((mapped.len() as f32) / (num_columns as f32)).ceil() as usize;

    for y in 0..height {
        for x in 0..num_columns {
            let idx = y + (x * height);
            let state = mapped.get(idx);
            if let Some(state) = state {
                let our_width = state.name.len() + state.version.len() + 3;
                let print_width = largest_width - our_width;
                if x == num_columns - 1 {
                    print!(
                        "{} {:width$}{}",
                        state.name.clone().bold(),
                        " ",
                        state.version.clone().magenta(),
                        width = print_width
                    )
                } else {
                    print!(
                        "{} {:width$}{}   ",
                        state.name.clone().bold(),
                        " ",
                        state.version.clone().magenta(),
                        width = print_width
                    );
                }
            }
        }
        println!();
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct State {
    name: String,
    version: String,
}
