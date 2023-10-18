// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{io, path::PathBuf};

use bytes::Bytes;
use futures::{stream::BoxStream, Stream, StreamExt};
use once_cell::sync::Lazy;
use thiserror::Error;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use url::Url;

/// Shared client for tcp socket reuse and connection limit
static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::ClientBuilder::new()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .expect("build reqwest client")
});

/// Fetch a resource at the provided [`Url`] and stream it's response bytes
pub async fn get(url: Url) -> Result<BoxStream<'static, Result<Bytes, Error>>, Error> {
    match url_file(&url) {
        Some(path) => Ok(read(path).await?.boxed()),
        _ => Ok(fetch(url).await?.boxed()),
    }
}

async fn fetch(url: Url) -> Result<impl Stream<Item = Result<Bytes, Error>>, Error> {
    let response = CLIENT.get(url).send().await?;

    response
        .error_for_status()
        .map(reqwest::Response::bytes_stream)
        .map(|stream| stream.map(|result| result.map_err(Error::Fetch)))
        .map_err(Error::Fetch)
}

async fn read(path: PathBuf) -> Result<impl Stream<Item = Result<Bytes, Error>>, Error> {
    // 4 MiB
    const BUFFER_SIZE: usize = 4 * 1024 * 1024;

    let file = File::open(path).await?;

    Ok(ReaderStream::with_capacity(file, BUFFER_SIZE).map(|result| result.map_err(Error::Read)))
}

fn url_file(url: &Url) -> Option<PathBuf> {
    if url.scheme() == "file" {
        url.to_file_path().ok()
    } else {
        None
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("fetch error: {0}")]
    Fetch(#[from] reqwest::Error),
    #[error("read error: {0}")]
    Read(#[from] io::Error),
}
