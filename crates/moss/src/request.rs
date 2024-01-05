// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{io, path::PathBuf};

use bytes::Bytes;
use futures::{
    stream::{self, BoxStream},
    Stream, StreamExt,
};
use once_cell::sync::Lazy;
use thiserror::Error;
use tokio::{fs::File, io::AsyncReadExt};
use tokio_util::io::ReaderStream;
use url::Url;

use crate::environment;

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
        Some(path) => read(path).await,
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

async fn read(path: PathBuf) -> Result<BoxStream<'static, Result<Bytes, Error>>, Error> {
    let mut file = File::open(path).await?;
    let size = file.metadata().await?.len() as usize;

    if size > environment::FILE_READ_CHUNK_THRESHOLD {
        let stream = ReaderStream::with_capacity(file, environment::FILE_READ_BUFFER_SIZE);

        Ok(stream.map(|result| result.map_err(Error::Read)).boxed())
    } else {
        let mut bytes = Vec::with_capacity(size);
        file.read_to_end(&mut bytes).await?;

        Ok(stream::once(async move { Ok(bytes.into()) }).boxed())
    }
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
    #[error("fetch")]
    Fetch(#[from] reqwest::Error),
    #[error("io")]
    Read(#[from] io::Error),
}
