use std::{io, path::PathBuf};

use futures::StreamExt;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use stone::read::Payload;
use thiserror::Error;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    task,
};
use url::Url;

use crate::{
    package::{Id, Meta},
    request, Installation,
};

/// Fetch a package with the provided [`Meta`] and [`Installation`] and return a [`Download`] on success.
pub async fn fetch(meta: &Meta, installation: &Installation) -> Result<Download, Error> {
    let url = meta.uri.as_ref().ok_or(Error::MissingUri)?.parse::<Url>()?;
    let hash = meta.hash.as_ref().ok_or(Error::MissingHash)?;

    let mut bytes = request::get(url).await.unwrap();

    let download_path = download_path(installation, hash).await?;
    let mut out = File::create(&download_path).await?;

    while let Some(chunk) = bytes.next().await {
        out.write_all(&chunk?).await?;
    }

    out.flush().await?;

    Ok(Download {
        id: meta.id().into(),
        path: download_path,
        installation: installation.clone(),
    })
}

/// A package that has been downloaded to the installation
pub struct Download {
    id: Id,
    path: PathBuf,
    installation: Installation,
}

impl Download {
    /// Unpack the downloaded package
    // TODO: Return an "Unpacked" struct which has a "blit" method on it?
    pub async fn unpack(self) -> Result<(), Error> {
        task::spawn_blocking(move || {
            use std::fs::{create_dir_all, remove_file, File};
            use std::io::{copy, Read, Seek, SeekFrom};

            let content_dir = self.installation.cache_path("content");
            let content_path = content_dir.join(self.id.as_ref());

            create_dir_all(&content_dir)?;

            let mut reader = stone::read(File::open(&self.path)?)?;

            let payloads = reader.payloads()?.collect::<Result<Vec<_>, _>>()?;
            let content = payloads
                .iter()
                .find_map(Payload::content)
                .ok_or(Error::MissingContent)?;

            let content_file = File::options()
                .read(true)
                .write(true)
                .create(true)
                .open(&content_path)?;

            reader.unpack_content(content, &mut &content_file)?;

            payloads
                .par_iter()
                .filter_map(Payload::index)
                .flatten()
                .map(|idx| {
                    // Split file reader over index range
                    let mut file = &content_file;
                    file.seek(SeekFrom::Start(idx.start))?;
                    let mut split_file = (&mut file).take(idx.end - idx.start);

                    let path = asset_path(&self.installation, &format!("{:02x}", idx.digest))?;

                    let mut output = File::create(path)?;

                    copy(&mut split_file, &mut output)?;

                    Ok(())
                })
                .collect::<Result<Vec<_>, Error>>()?;

            remove_file(&content_path)?;

            Ok(())
        })
        .await
        .expect("join handle")
    }
}

async fn download_path(installation: &Installation, hash: &str) -> Result<PathBuf, Error> {
    if hash.len() < 5 {
        return Err(Error::MalformedHash(hash.to_string()));
    }

    let directory = installation
        .cache_path("downloads")
        .join("v1")
        .join(&hash[..5])
        .join(&hash[hash.len() - 5..]);

    if !directory.exists() {
        fs::create_dir_all(&directory).await?;
    }

    Ok(directory.join(hash))
}

fn asset_path(installation: &Installation, hash: &str) -> Result<PathBuf, Error> {
    let directory = if hash.len() >= 10 {
        installation
            .assets_path("v2")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(&hash[4..6])
    } else {
        installation.assets_path("v2")
    };

    if !directory.exists() {
        std::fs::create_dir_all(&directory)?;
    }

    Ok(directory.join(hash))
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("stone format: {0}")]
    Format(#[from] stone::read::Error),
    #[error("missing download hash")]
    MissingHash,
    #[error("missing download URI")]
    MissingUri,
    #[error("missing content payload")]
    MissingContent,
    #[error("malformed download hash: {0}")]
    MalformedHash(String),
    #[error("invalid url: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
