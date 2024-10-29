use stone::{StonePayloadLayout, StonePayloadLayoutEntry, StonePayloadLayoutFileType};

use crate::StoneString;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct StonePayloadLayoutRecord {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub tag: u32,
    pub file_type: StonePayloadLayoutFileType,
    pub file_payload: StonePayloadLayoutFilePayload,
}

impl From<&StonePayloadLayout> for StonePayloadLayoutRecord {
    fn from(record: &StonePayloadLayout) -> Self {
        StonePayloadLayoutRecord {
            uid: record.uid,
            gid: record.gid,
            mode: record.mode,
            tag: record.tag,
            file_type: record.entry.file_type(),
            file_payload: match &record.entry {
                StonePayloadLayoutEntry::Regular(hash, name) => StonePayloadLayoutFilePayload {
                    regular: StonePayloadLayoutFileRegular {
                        hash: hash.to_be_bytes(),
                        name: StoneString::new(name),
                    },
                },
                StonePayloadLayoutEntry::Symlink(source, target) => StonePayloadLayoutFilePayload {
                    symlink: StonePayloadLayoutFileSymlink {
                        source: StoneString::new(source),
                        target: StoneString::new(target),
                    },
                },
                StonePayloadLayoutEntry::Directory(name) => StonePayloadLayoutFilePayload {
                    directory: StoneString::new(name),
                },
                StonePayloadLayoutEntry::CharacterDevice(name) => StonePayloadLayoutFilePayload {
                    character_device: StoneString::new(name),
                },
                StonePayloadLayoutEntry::BlockDevice(name) => StonePayloadLayoutFilePayload {
                    block_device: StoneString::new(name),
                },
                StonePayloadLayoutEntry::Fifo(name) => StonePayloadLayoutFilePayload {
                    fifo: StoneString::new(name),
                },
                StonePayloadLayoutEntry::Socket(name) => StonePayloadLayoutFilePayload {
                    socket: StoneString::new(name),
                },
            },
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union StonePayloadLayoutFilePayload {
    regular: StonePayloadLayoutFileRegular,
    symlink: StonePayloadLayoutFileSymlink,
    directory: StoneString,
    character_device: StoneString,
    block_device: StoneString,
    fifo: StoneString,
    socket: StoneString,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct StonePayloadLayoutFileRegular {
    pub hash: [u8; 16],
    pub name: StoneString,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct StonePayloadLayoutFileSymlink {
    pub source: StoneString,
    pub target: StoneString,
}
