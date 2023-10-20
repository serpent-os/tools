// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

/// Max concurrency for disk tasks
pub const MAX_DISK_CONCURRENCY: usize = 16;
/// Max concurrency for network tasks
pub const MAX_NETWORK_CONCURRENCY: usize = 8;
/// Buffer size used when reading a file, 16KiB
pub const FILE_READ_BUFFER_SIZE: usize = 16 * 1024;
/// DB batch size
pub const DB_BATCH_SIZE: usize = 1000;
