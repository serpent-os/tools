// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

pub mod subgraph;

pub use self::reexport::*;

pub mod reexport {
    pub use petgraph::graph::DiGraph;
}
