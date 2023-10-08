// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use bytes::Bytes;
use futures::Stream;
use once_cell::sync::Lazy;
use reqwest::Result;
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
pub async fn get(url: Url) -> Result<impl Stream<Item = Result<Bytes>>> {
    let response = CLIENT.get(url).send().await?;

    response
        .error_for_status()
        .map(reqwest::Response::bytes_stream)
}
