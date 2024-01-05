// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use url::Url;

use crate::{package, repository};

/// What system (domain) does this job operate in?
#[derive(Clone, Debug)]
pub enum Domain {
    Package(package::Id),
    Repository(repository::Id),
}

/// Allow us to handle various hash types in future
#[derive(Clone, Debug)]
pub enum CheckType {
    /// Verify hashsum using the SHA256 method
    Sha256(String),
}

/// From whence this Job came
#[derive(Clone, Debug)]
pub enum Origin {
    /// Locally available
    LocalFile(PathBuf),

    /// Must be fetched from a remote URI
    RemoteFile(Url),
}

/// A job is used to describe the operation required to get some pkgID installed locally
#[derive(Clone, Debug)]
pub struct Job {
    /// Domain this job is operating on
    pub domain: Domain,

    /// Where are we getting this from.. ?
    pub origin: Origin,

    /// How do we verify the download?
    pub check: Option<CheckType>,

    // How large (in bytes) is the download?
    pub size: u64,
}
