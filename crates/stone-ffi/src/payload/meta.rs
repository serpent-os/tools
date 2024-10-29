use stone::{StonePayloadMetaDependency, StonePayloadMetaTag};

use crate::StoneString;

#[repr(C)]
pub struct StonePayloadMetaRecord {
    pub tag: StonePayloadMetaTag,
    pub primitive_type: StonePayloadMetaPrimitiveType,
    pub primitive_payload: StonePayloadMetaPrimitivePayload,
}

impl From<&stone::StonePayloadMeta> for StonePayloadMetaRecord {
    fn from(record: &stone::StonePayloadMeta) -> Self {
        Self {
            tag: record.tag,
            primitive_type: match &record.kind {
                stone::StonePayloadMetaKind::Int8(_) => StonePayloadMetaPrimitiveType::Int8,
                stone::StonePayloadMetaKind::Uint8(_) => StonePayloadMetaPrimitiveType::Uint8,
                stone::StonePayloadMetaKind::Int16(_) => StonePayloadMetaPrimitiveType::Int16,
                stone::StonePayloadMetaKind::Uint16(_) => StonePayloadMetaPrimitiveType::Uint16,
                stone::StonePayloadMetaKind::Int32(_) => StonePayloadMetaPrimitiveType::Int32,
                stone::StonePayloadMetaKind::Uint32(_) => StonePayloadMetaPrimitiveType::Uint32,
                stone::StonePayloadMetaKind::Int64(_) => StonePayloadMetaPrimitiveType::Int64,
                stone::StonePayloadMetaKind::Uint64(_) => StonePayloadMetaPrimitiveType::Uint64,
                stone::StonePayloadMetaKind::String(_) => StonePayloadMetaPrimitiveType::String,
                stone::StonePayloadMetaKind::Dependency(_, _) => StonePayloadMetaPrimitiveType::Dependency,
                stone::StonePayloadMetaKind::Provider(_, _) => StonePayloadMetaPrimitiveType::Provider,
            },
            primitive_payload: match &record.kind {
                stone::StonePayloadMetaKind::Int8(a) => StonePayloadMetaPrimitivePayload { int8: *a },
                stone::StonePayloadMetaKind::Uint8(a) => StonePayloadMetaPrimitivePayload { uint8: *a },
                stone::StonePayloadMetaKind::Int16(a) => StonePayloadMetaPrimitivePayload { int16: *a },
                stone::StonePayloadMetaKind::Uint16(a) => StonePayloadMetaPrimitivePayload { uint16: *a },
                stone::StonePayloadMetaKind::Int32(a) => StonePayloadMetaPrimitivePayload { int32: *a },
                stone::StonePayloadMetaKind::Uint32(a) => StonePayloadMetaPrimitivePayload { uint32: *a },
                stone::StonePayloadMetaKind::Int64(a) => StonePayloadMetaPrimitivePayload { int64: *a },
                stone::StonePayloadMetaKind::Uint64(a) => StonePayloadMetaPrimitivePayload { uint64: *a },
                stone::StonePayloadMetaKind::String(a) => StonePayloadMetaPrimitivePayload {
                    string: StoneString::new(a),
                },
                stone::StonePayloadMetaKind::Dependency(kind, name) => StonePayloadMetaPrimitivePayload {
                    dependency: StonePayloadMetaDependencyValue {
                        kind: *kind,
                        name: StoneString::new(name),
                    },
                },
                stone::StonePayloadMetaKind::Provider(kind, name) => StonePayloadMetaPrimitivePayload {
                    provider: StonePayloadMetaProviderValue {
                        kind: *kind,
                        name: StoneString::new(name),
                    },
                },
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum StonePayloadMetaPrimitiveType {
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Int64,
    Uint64,
    String,
    Dependency,
    Provider,
}

#[repr(C)]
pub union StonePayloadMetaPrimitivePayload {
    int8: i8,
    uint8: u8,
    int16: i16,
    uint16: u16,
    int32: i32,
    uint32: u32,
    int64: i64,
    uint64: u64,
    string: StoneString,
    dependency: StonePayloadMetaDependencyValue,
    provider: StonePayloadMetaProviderValue,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct StonePayloadMetaDependencyValue {
    pub kind: StonePayloadMetaDependency,
    pub name: StoneString,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct StonePayloadMetaProviderValue {
    pub kind: StonePayloadMetaDependency,
    pub name: StoneString,
}
