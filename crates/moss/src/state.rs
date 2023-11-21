// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fmt, io::Write};

use chrono::{DateTime, Utc};
use tui::{pretty, Stylize};

use crate::package;

/// Unique identifier for [`State`]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Id(i64);

impl Id {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl From<i64> for Id {
    fn from(id: i64) -> Self {
        Id(id)
    }
}

impl From<Id> for i64 {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// State types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    /// Automatically constructed state
    Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// Unique identifier for this state
    pub id: Id,
    /// Quick summary for the state (optional)
    pub summary: Option<String>,
    /// Description for the state (optional)
    pub description: Option<String>,
    /// Selections in this state
    pub selections: Vec<Selection>,
    /// Creation timestamp
    pub created: DateTime<Utc>,
    /// Relevant type for this State
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub package: package::Id,
    /// Marks whether the package was explicitly installed
    /// by the user, or if it's a "transitive" dependency
    pub explicit: bool,
    pub reason: Option<String>,
}

impl Selection {
    pub fn explicit(package: package::Id) -> Self {
        Self {
            package,
            explicit: true,
            reason: None,
        }
    }

    pub fn transitive(package: package::Id) -> Self {
        Self {
            package,
            explicit: true,
            reason: None,
        }
    }

    pub fn reason(self, reason: impl ToString) -> Self {
        Self {
            reason: Some(reason.to_string()),
            ..self
        }
    }
}

pub struct ColumnDisplay<'a>(pub &'a State);

impl<'a> pretty::ColumnDisplay for ColumnDisplay<'a> {
    fn get_display_width(&self) -> usize {
        const WHITESPACE: usize = 1;

        "State ".len() + self.0.id.to_string().len()
    }

    fn display_column(&self, writer: &mut impl Write, col: pretty::Column, width: usize) {
        let right_gap = matches!(col, pretty::Column::Last)
            .then_some("   ")
            .unwrap_or_default();

        let _ = write!(
            writer,
            "State {}{:width$}",
            self.0.id.to_string().bold(),
            " ",
        );
    }
}
