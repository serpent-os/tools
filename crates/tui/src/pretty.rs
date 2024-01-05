// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Pretty printing for moss CLI

use std::{
    cmp::max,
    io::{stdout, Write},
};

use crate::term_size;

/// Simplistic handling of renderable display columns
/// allowing implementations to handle first, n and last specific alignment
pub enum Column {
    First,
    Nth(usize),
    Last,
}

/// Implementing ColumnDisplay for a type allows use of the print_to_columns
/// function, ie pretty print in individual alphabetically sorted columns
pub trait ColumnDisplay: Sized {
    /// Implementations return their full display size
    fn get_display_width(&self) -> usize;

    /// Render to the given Writer
    fn display_column(&self, writer: &mut impl Write, col: Column, width: usize);
}

/// Print a vec of items that implement the ColumnDisplay trait.
/// These will be printed in individual columns assuming that the input order is
/// alphabetically sorted, to give each column an ascending alpha sort.
pub fn print_to_columns<T: ColumnDisplay>(items: &[T]) {
    let terminal_width = term_size().width;

    // Figure render constraints
    let largest_element = items
        .iter()
        .max_by_key(|p| p.get_display_width() + 3)
        .unwrap();
    let largest_width = largest_element.get_display_width() + 6;
    let num_columns = max(1, terminal_width / largest_width);
    let height = ((items.len() as f32) / (num_columns as f32)).ceil() as usize;

    let mut stdout = stdout().lock();

    for y in 0..height {
        for x in 0..num_columns {
            let idx = y + (x * height);
            let state = items.get(idx);
            if let Some(state) = state {
                let column = if x == 0 {
                    Column::First
                } else if x == num_columns - 1 {
                    Column::Last
                } else {
                    Column::Nth(x)
                };
                state.display_column(
                    &mut stdout,
                    column,
                    largest_width - state.get_display_width(),
                );
            }
        }
        println!();
    }
}
