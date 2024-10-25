#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <stone.h>
#include <unistd.h>

// 32 byte stone header
static uint8_t HEADER_BUF[] = {0x00, 0x6d, 0x6f, 0x73, 0x00, 0x04, 0x00, 0x00,
                               0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x03, 0x00,
                               0x00, 0x04, 0x00, 0x00, 0x05, 0x00, 0x00, 0x06,
                               0x00, 0x00, 0x07, 0x01, 0x00, 0x00, 0x00, 0x01};

void print_header_v1(StoneHeaderV1 *header) {
  char file_type[100];

  stone_format_header_v1_file_type(header->file_type, file_type);

  printf("StoneHeaderV1 {\n");
  printf("  num_payloads: %d\n", header->num_payloads);
  printf("  file_type: %s\n", file_type);
  printf("}\n");
}

void print_payload_header(StonePayloadHeader *header) {
  char compression[100];
  char kind[100];

  stone_format_payload_compression(header->compression, compression);
  stone_format_payload_kind(header->kind, &(kind[0]));

  printf("StonePayload {\n");
  printf("  kind: %s\n", kind);
  printf("  plain_size: %d\n", header->plain_size);
  printf("  stored_size: %d\n", header->stored_size);
  printf("  compression: %s\n", compression);
  printf("  num_records: %d\n", header->num_records);
  printf("  version: %d\n", header->version);
  printf("}\n");
}

void print_payload_layout_record(StonePayloadLayoutRecord *record) {
  char file_type[100];

  stone_format_payload_layout_file_type(record->file_type, file_type);

  printf("StonePayloadLayoutRecord {\n");
  printf("  uid: %d\n", record->uid);
  printf("  gid: %d\n", record->gid);
  printf("  mode: %d\n", record->mode);
  printf("  tag: %d\n", record->tag);
  printf("  file_type: %s\n", file_type);

  switch (record->file_type) {
  case STONE_PAYLOAD_LAYOUT_FILE_TYPE_REGULAR: {
    printf("  hash: ");
    for (size_t i = 0; i < 16; i++) {
      printf("%.02x", record->file_payload.regular.hash[i]);
    }
    printf("\n  name: %.*s\n", record->file_payload.regular.name.size,
           record->file_payload.regular.name.buf);
    break;
  }
  case STONE_PAYLOAD_LAYOUT_FILE_TYPE_SYMLINK: {
    printf("  source: %.*s\n", record->file_payload.symlink.source.size,
           record->file_payload.symlink.source.buf);
    printf("  target: %.*s\n", record->file_payload.symlink.target.size,
           record->file_payload.symlink.target.buf);
    break;
  }
  default:
  }

  printf("}\n");
}

void process_reader(StoneReader *reader, StoneHeaderVersion version) {
  StoneHeaderV1 header;
  StonePayload *payload;

  assert(version == STONE_HEADER_VERSION_V1);

  stone_reader_header_v1(reader, &header);
  print_header_v1(&header);

  while (stone_reader_next_payload(reader, &payload) >= 0) {
    StonePayloadHeader payload_header;

    stone_payload_header(payload, &payload_header);
    print_payload_header(&payload_header);

    switch (payload_header.kind) {
    case STONE_PAYLOAD_KIND_LAYOUT: {
      StonePayloadLayoutRecord record;

      while (stone_payload_next_layout_record(payload, &record) >= 0) {
        print_payload_layout_record(&record);
      }

      break;
    }
    default:
    }

    stone_payload_destroy(payload);
  }

  stone_reader_destroy(reader);
}

int main(int argc, char *argv[]) {
  FILE *fptr;
  StoneReader *reader;
  StoneHeaderVersion version;
  char *file = "./test/bash-completion-2.11-1-1-x86_64.stone";

  printf("Reading stone from '%s'\n\n", file);
  fptr = fopen(file, "r");
  stone_reader_read_file(fileno(fptr), &reader, &version);
  process_reader(reader, version);

  printf("\n");
  printf("Reading stone header from buffer\n\n");
  stone_reader_read_buf(HEADER_BUF, sizeof(HEADER_BUF), &reader, &version);
  process_reader(reader, version);

  return 0;
}
