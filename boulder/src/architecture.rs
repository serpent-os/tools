// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use derive_more::Display;

pub const fn host() -> Architecture {
    #[cfg(target_arch = "x86_64")]
    {
        Architecture::X86_64
    }
    #[cfg(target_arch = "x86")]
    {
        Architecture::X86
    }
    #[cfg(target_arch = "aarch64")]
    {
        Architecture::Aarch64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum Architecture {
    X86_64,
    X86_64_V3X,
    X86,
    Aarch64,
}

impl Architecture {
    pub fn supports_emul32(&self) -> bool {
        match self {
            Architecture::X86_64 => true,
            Architecture::X86_64_V3X => false,
            Architecture::X86 => false,
            Architecture::Aarch64 => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Display)]
pub enum BuildTarget {
    #[display(fmt = "{_0}")]
    Native(Architecture),
    #[display(fmt = "x86_64-v3x")]
    X86_64_v3x(Architecture),
    #[display(fmt = "emul32/{_0}")]
    Emul32(Architecture),
}

impl BuildTarget {
    pub fn emul32(&self) -> bool {
        matches!(self, BuildTarget::Emul32(_))
    }

    pub fn x86_64_v3x(&self) -> bool {
        matches!(self, BuildTarget::X86_64_v3x(_))
    }

    pub fn host_architecture(&self) -> Architecture {
        match self {
            BuildTarget::Native(arch) => *arch,
            BuildTarget::X86_64_v3x(arch) => *arch,
            BuildTarget::Emul32(arch) => *arch,
        }
    }
}
