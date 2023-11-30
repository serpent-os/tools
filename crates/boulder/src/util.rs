// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{io, path::Path};

use futures::{future::BoxFuture, FutureExt};
use tokio::fs::{copy, create_dir_all, read_dir, read_link, remove_dir_all, symlink};

pub async fn ensure_dir_exists(path: &Path) -> Result<(), io::Error> {
    if !path.exists() {
        create_dir_all(path).await?;
    }
    Ok(())
}

pub async fn recreate_dir(path: &Path) -> Result<(), io::Error> {
    if path.exists() {
        remove_dir_all(path).await?;
    }
    create_dir_all(path).await?;
    Ok(())
}

pub fn copy_dir<'a>(
    source_dir: &'a Path,
    out_dir: &'a Path,
) -> BoxFuture<'a, Result<(), io::Error>> {
    async move {
        recreate_dir(out_dir).await?;

        let mut contents = read_dir(&source_dir).await?;

        while let Some(entry) = contents.next_entry().await? {
            let path = entry.path();

            if let Some(file_name) = path.file_name() {
                let dest = out_dir.join(file_name);
                let meta = entry.metadata().await?;

                if meta.is_dir() {
                    copy_dir(&path, &dest).await?;
                } else if meta.is_file() {
                    copy(&path, &dest).await?;
                } else if meta.is_symlink() {
                    symlink(read_link(&path).await?, &dest).await?;
                }
            }
        }

        Ok(())
    }
    .boxed()
}
