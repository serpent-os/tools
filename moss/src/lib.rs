// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

pub use self::dependency::{Dependency, Provider};
pub use self::registry::Registry;

pub mod cli;
pub mod client;
pub mod dependency;
pub mod registry;
