// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Pretty printing for moss CLI

use std::{cmp::max, iter::zip};

use itertools::Itertools;
use moss::Package;
use tui::Stylize;

/// Print packages as column output
pub fn print_to_columns<T>(items: T)
where
    T: IntoIterator<Item = Package>,
{
    // TODO: Get real constraints
    const TERMINAL_WIDTH: usize = 80;

    let mut mapped = items
        .into_iter()
        .map(|p| State {
            name: p.meta.name.to_string(),
            version: p.meta.version_identifier,
        })
        .collect_vec();
    mapped.sort();

    let largest_element = mapped
        .iter()
        .max_by_key(|p| p.name.len() + p.version.len() + 3)
        .unwrap();
    let largest_width = largest_element.name.len() + largest_element.version.len() + 6;
    let num_columns = max(1, TERMINAL_WIDTH / largest_width);

    let mut cleared = false;
    let screen_wide = (0..num_columns).cycle();
    for (state, x) in zip(mapped, screen_wide) {
        let our_width = state.name.len() + state.version.len() + 3;
        let print_width = largest_width - our_width;
        if x == num_columns - 1 {
            cleared = true;
            println!("{} - {}", state.name.bold(), state.version.magenta())
        } else {
            print!(
                "{} - {}{:width$}",
                state.name.bold(),
                state.version.magenta(),
                " ",
                width = print_width
            );
            cleared = false;
        }
    }
    if !cleared {
        println!();
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct State {
    name: String,
    version: String,
}
