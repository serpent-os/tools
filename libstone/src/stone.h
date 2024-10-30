// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0


#ifndef STONE_H
#define STONE_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Well known file type for a v1 stone container
 *
 * Some types are now legacy as we're going to use Ion to define them.
 *
 */
enum StoneHeaderV1FileType {
  /**
   * Binary package
   */
  STONE_HEADER_V1_FILE_TYPE_BINARY = 1,
  /**
   * Delta package
   */
  STONE_HEADER_V1_FILE_TYPE_DELTA,
  /**
   * (Legacy) repository index
   */
  STONE_HEADER_V1_FILE_TYPE_REPOSITORY,
  /**
   * (Legacy) build manifest
   */
  STONE_HEADER_V1_FILE_TYPE_BUILD_MANIFEST,
};
typedef uint8_t StoneHeaderV1FileType;

/**
 * Format versions are defined as u32, to allow further mangling in future
 */
enum StoneHeaderVersion {
  STONE_HEADER_VERSION_V1 = 1,
};
typedef uint32_t StoneHeaderVersion;

enum StonePayloadCompression {
  STONE_PAYLOAD_COMPRESSION_NONE = 1,
  STONE_PAYLOAD_COMPRESSION_ZSTD = 2,
};
typedef uint8_t StonePayloadCompression;

enum StonePayloadKind {
  STONE_PAYLOAD_KIND_META = 1,
  STONE_PAYLOAD_KIND_CONTENT = 2,
  STONE_PAYLOAD_KIND_LAYOUT = 3,
  STONE_PAYLOAD_KIND_INDEX = 4,
  STONE_PAYLOAD_KIND_ATTRIBUTES = 5,
  STONE_PAYLOAD_KIND_DUMB = 6,
};
typedef uint8_t StonePayloadKind;

/**
 * Layout entries record their target file type so they can be rebuilt on
 * the target installation.
 */
enum StonePayloadLayoutFileType {
  /**
   * Regular file
   */
  STONE_PAYLOAD_LAYOUT_FILE_TYPE_REGULAR = 1,
  /**
   * Symbolic link (source + target set)
   */
  STONE_PAYLOAD_LAYOUT_FILE_TYPE_SYMLINK,
  /**
   * Directory node
   */
  STONE_PAYLOAD_LAYOUT_FILE_TYPE_DIRECTORY,
  /**
   * Character device
   */
  STONE_PAYLOAD_LAYOUT_FILE_TYPE_CHARACTER_DEVICE,
  /**
   * Block device
   */
  STONE_PAYLOAD_LAYOUT_FILE_TYPE_BLOCK_DEVICE,
  /**
   * FIFO node
   */
  STONE_PAYLOAD_LAYOUT_FILE_TYPE_FIFO,
  /**
   * UNIX Socket
   */
  STONE_PAYLOAD_LAYOUT_FILE_TYPE_SOCKET,
};
typedef uint8_t StonePayloadLayoutFileType;

enum StonePayloadMetaDependency {
  /**
   * Just the plain name of a package
   */
  STONE_PAYLOAD_META_DEPENDENCY_PACKAGE_NAME = 0,
  /**
   * A soname based dependency
   */
  STONE_PAYLOAD_META_DEPENDENCY_SHARED_LIBRARY,
  /**
   * A pkgconfig `.pc` based dependency
   */
  STONE_PAYLOAD_META_DEPENDENCY_PKG_CONFIG,
  /**
   * Special interpreter (PT_INTERP/etc) to run the binaries
   */
  STONE_PAYLOAD_META_DEPENDENCY_INTERPRETER,
  /**
   * A CMake module
   */
  STONE_PAYLOAD_META_DEPENDENCY_C_MAKE,
  /**
   * A Python module
   */
  STONE_PAYLOAD_META_DEPENDENCY_PYTHON,
  /**
   * A binary in /usr/bin
   */
  STONE_PAYLOAD_META_DEPENDENCY_BINARY,
  /**
   * A binary in /usr/sbin
   */
  STONE_PAYLOAD_META_DEPENDENCY_SYSTEM_BINARY,
  /**
   * An emul32-compatible pkgconfig .pc dependency (lib32/*.pc)
   */
  STONE_PAYLOAD_META_DEPENDENCY_PKG_CONFIG32,
};
typedef uint8_t StonePayloadMetaDependency;

typedef enum StonePayloadMetaPrimitiveType {
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_INT8,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_UINT8,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_INT16,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_UINT16,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_INT32,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_UINT32,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_INT64,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_UINT64,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_STRING,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_DEPENDENCY,
  STONE_PAYLOAD_META_PRIMITIVE_TYPE_PROVIDER,
} StonePayloadMetaPrimitiveType;

enum StonePayloadMetaTag {
  STONE_PAYLOAD_META_TAG_NAME = 1,
  STONE_PAYLOAD_META_TAG_ARCHITECTURE = 2,
  STONE_PAYLOAD_META_TAG_VERSION = 3,
  STONE_PAYLOAD_META_TAG_SUMMARY = 4,
  STONE_PAYLOAD_META_TAG_DESCRIPTION = 5,
  STONE_PAYLOAD_META_TAG_HOMEPAGE = 6,
  STONE_PAYLOAD_META_TAG_SOURCE_ID = 7,
  STONE_PAYLOAD_META_TAG_DEPENDS = 8,
  STONE_PAYLOAD_META_TAG_PROVIDES = 9,
  STONE_PAYLOAD_META_TAG_CONFLICTS = 10,
  STONE_PAYLOAD_META_TAG_RELEASE = 11,
  STONE_PAYLOAD_META_TAG_LICENSE = 12,
  STONE_PAYLOAD_META_TAG_BUILD_RELEASE = 13,
  STONE_PAYLOAD_META_TAG_PACKAGE_URI = 14,
  STONE_PAYLOAD_META_TAG_PACKAGE_HASH = 15,
  STONE_PAYLOAD_META_TAG_PACKAGE_SIZE = 16,
  STONE_PAYLOAD_META_TAG_BUILD_DEPENDS = 17,
  STONE_PAYLOAD_META_TAG_SOURCE_URI = 18,
  STONE_PAYLOAD_META_TAG_SOURCE_PATH = 19,
  STONE_PAYLOAD_META_TAG_SOURCE_REF = 20,
};
typedef uint16_t StonePayloadMetaTag;

typedef struct StonePayload StonePayload;

typedef struct StoneReader StoneReader;

/**
 * Header for the v1 format version
 */
typedef struct StoneHeaderV1 {
  uint16_t num_payloads;
  StoneHeaderV1FileType file_type;
} StoneHeaderV1;

typedef struct StonePayloadHeader {
  uint64_t stored_size;
  uint64_t plain_size;
  uint8_t checksum[8];
  uintptr_t num_records;
  uint16_t version;
  StonePayloadKind kind;
  StonePayloadCompression compression;
} StonePayloadHeader;

typedef struct StoneString {
  const uint8_t *buf;
  size_t size;
} StoneString;

typedef struct StonePayloadLayoutFileRegular {
  uint8_t hash[16];
  struct StoneString name;
} StonePayloadLayoutFileRegular;

typedef struct StonePayloadLayoutFileSymlink {
  struct StoneString source;
  struct StoneString target;
} StonePayloadLayoutFileSymlink;

typedef union StonePayloadLayoutFilePayload {
  struct StonePayloadLayoutFileRegular regular;
  struct StonePayloadLayoutFileSymlink symlink;
  struct StoneString directory;
  struct StoneString character_device;
  struct StoneString block_device;
  struct StoneString fifo;
  struct StoneString socket;
} StonePayloadLayoutFilePayload;

typedef struct StonePayloadLayoutRecord {
  uint32_t uid;
  uint32_t gid;
  uint32_t mode;
  uint32_t tag;
  StonePayloadLayoutFileType file_type;
  union StonePayloadLayoutFilePayload file_payload;
} StonePayloadLayoutRecord;

typedef struct StonePayloadMetaDependencyValue {
  StonePayloadMetaDependency kind;
  struct StoneString name;
} StonePayloadMetaDependencyValue;

typedef struct StonePayloadMetaProviderValue {
  StonePayloadMetaDependency kind;
  struct StoneString name;
} StonePayloadMetaProviderValue;

typedef union StonePayloadMetaPrimitivePayload {
  int8_t int8;
  uint8_t uint8;
  int16_t int16;
  uint16_t uint16;
  int32_t int32;
  uint32_t uint32;
  int64_t int64;
  uint64_t uint64;
  struct StoneString string;
  struct StonePayloadMetaDependencyValue dependency;
  struct StonePayloadMetaProviderValue provider;
} StonePayloadMetaPrimitivePayload;

typedef struct StonePayloadMetaRecord {
  StonePayloadMetaTag tag;
  enum StonePayloadMetaPrimitiveType primitive_type;
  union StonePayloadMetaPrimitivePayload primitive_payload;
} StonePayloadMetaRecord;

typedef struct StonePayloadIndexRecord {
  uint64_t start;
  uint64_t end;
  uint8_t digest[16];
} StonePayloadIndexRecord;

typedef struct StonePayloadAttributeRecord {
  uintptr_t key_size;
  const uint8_t *key_buf;
  uintptr_t value_size;
  const uint8_t *value_buf;
} StonePayloadAttributeRecord;



int stone_reader_read_file(int file, struct StoneReader **reader_ptr, StoneHeaderVersion *version);

int stone_reader_read_buf(const uint8_t *buf,
                          uintptr_t len,
                          struct StoneReader **reader_ptr,
                          StoneHeaderVersion *version);

int stone_reader_header_v1(const struct StoneReader *reader, struct StoneHeaderV1 *header);

int stone_reader_next_payload(struct StoneReader *reader, struct StonePayload **payload_ptr);

int stone_reader_unpack_content_payload_to_file(struct StoneReader *reader,
                                                const struct StonePayload *payload,
                                                int file);

int stone_reader_unpack_content_payload_to_buf(struct StoneReader *reader,
                                               const struct StonePayload *payload,
                                               uint8_t *data);

void stone_reader_destroy(struct StoneReader *reader);

int stone_payload_header(const struct StonePayload *payload, struct StonePayloadHeader *header);

int stone_payload_next_layout_record(struct StonePayload *payload,
                                     struct StonePayloadLayoutRecord *record);

int stone_payload_next_meta_record(struct StonePayload *payload,
                                   struct StonePayloadMetaRecord *record);

int stone_payload_next_index_record(struct StonePayload *payload,
                                    struct StonePayloadIndexRecord *record);

int stone_payload_next_attribute_record(struct StonePayload *payload,
                                        struct StonePayloadAttributeRecord *record);

void stone_payload_destroy(struct StonePayload *payload);

void stone_format_header_v1_file_type(StoneHeaderV1FileType file_type, uint8_t *buf);

void stone_format_payload_compression(StonePayloadCompression compression, uint8_t *buf);

void stone_format_payload_kind(StonePayloadKind kind, uint8_t *buf);

void stone_format_payload_layout_file_type(StonePayloadLayoutFileType file_type, uint8_t *buf);

void stone_format_payload_meta_tag(StonePayloadMetaTag tag, uint8_t *buf);

void stone_format_payload_meta_dependency(StonePayloadMetaDependency dependency, uint8_t *buf);

#endif /* STONE_H */
