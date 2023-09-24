// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::Write;

use tui::{
    pretty::{Column, ColumnDisplay},
    Stylize,
};

use crate::Package;

/// Allow display packages in column form
impl ColumnDisplay for Package {
    fn get_display_width(&self) -> usize {
        self.meta.name.to_string().len() + self.meta.version_identifier.len() + 3
    }

    fn display_column(&self, writer: &mut impl Write, col: Column, width: usize) {
        let _ = match col {
            Column::Last => write!(
                writer,
                "{} {:width$}{}",
                self.meta.name.to_string().bold(),
                " ",
                self.meta.version_identifier.clone().magenta()
            ),
            _ => write!(
                writer,
                "{} {:width$}{}   ",
                self.meta.name.to_string().bold(),
                " ",
                self.meta.version_identifier.clone().magenta()
            ),
        };
    }
}
