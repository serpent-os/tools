// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    PackageName(String),
}

// TODO:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency(Kind);

// TODO:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider(Kind);
