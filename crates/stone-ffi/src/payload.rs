use std::mem::ManuallyDrop;

use stone::{StoneDecodedPayload, StonePayloadHeader, StonePayloadLayoutEntry, StonePayloadLayoutFileType};

use super::StoneString;

pub struct StonePayload {
    pub decoded: StoneDecodedPayload,
    next_record: usize,
}

impl StonePayload {
    pub fn header(&self) -> &StonePayloadHeader {
        self.decoded.header()
    }

    pub fn next_layout_record(&mut self) -> Option<StonePayloadLayoutRecord> {
        if self.next_record >= self.header().num_records {
            return None;
        }

        self.next_record += 1;

        let payload = self.decoded.layout()?;
        let record = payload.body.get(self.next_record)?;

        Some(StonePayloadLayoutRecord {
            uid: record.uid,
            gid: record.gid,
            mode: record.mode,
            tag: record.tag,
            file_type: record.entry.file_type(),
            file_payload: match &record.entry {
                StonePayloadLayoutEntry::Regular(hash, name) => StonePayloadLayoutFilePayload {
                    regular: ManuallyDrop::new(StonePayloadLayoutFileRegular {
                        hash: hash.to_be_bytes(),
                        name: StoneString::new(name),
                    }),
                },
                StonePayloadLayoutEntry::Symlink(source, target) => StonePayloadLayoutFilePayload {
                    symlink: ManuallyDrop::new(StonePayloadLayoutFileSymlink {
                        source: StoneString::new(source),
                        target: StoneString::new(target),
                    }),
                },
                StonePayloadLayoutEntry::Directory(name) => StonePayloadLayoutFilePayload {
                    directory: ManuallyDrop::new(StoneString::new(name)),
                },
                StonePayloadLayoutEntry::CharacterDevice(name) => StonePayloadLayoutFilePayload {
                    character_device: ManuallyDrop::new(StoneString::new(name)),
                },
                StonePayloadLayoutEntry::BlockDevice(name) => StonePayloadLayoutFilePayload {
                    block_device: ManuallyDrop::new(StoneString::new(name)),
                },
                StonePayloadLayoutEntry::Fifo(name) => StonePayloadLayoutFilePayload {
                    fifo: ManuallyDrop::new(StoneString::new(name)),
                },
                StonePayloadLayoutEntry::Socket(name) => StonePayloadLayoutFilePayload {
                    socket: ManuallyDrop::new(StoneString::new(name)),
                },
            },
        })
    }
}

impl From<StoneDecodedPayload> for StonePayload {
    fn from(decoded: StoneDecodedPayload) -> Self {
        Self {
            decoded,
            next_record: 0,
        }
    }
}

#[repr(C)]
pub struct StonePayloadLayoutRecord {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub tag: u32,
    pub file_type: StonePayloadLayoutFileType,
    pub file_payload: StonePayloadLayoutFilePayload,
}

#[repr(C)]
pub union StonePayloadLayoutFilePayload {
    regular: ManuallyDrop<StonePayloadLayoutFileRegular>,
    symlink: ManuallyDrop<StonePayloadLayoutFileSymlink>,
    directory: ManuallyDrop<StoneString>,
    character_device: ManuallyDrop<StoneString>,
    block_device: ManuallyDrop<StoneString>,
    fifo: ManuallyDrop<StoneString>,
    socket: ManuallyDrop<StoneString>,
}

#[repr(C)]
pub struct StonePayloadLayoutFileRegular {
    pub hash: [u8; 16],
    pub name: StoneString,
}

#[repr(C)]
pub struct StonePayloadLayoutFileSymlink {
    pub source: StoneString,
    pub target: StoneString,
}
