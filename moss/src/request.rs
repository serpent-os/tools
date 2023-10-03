use bytes::Bytes;
use futures::Stream;
use once_cell::sync::Lazy;
use reqwest::Result;
use tokio::sync::Semaphore;
use url::Url;

const MAX_CONNECTIONS: usize = 8;

/// Shared client for tcp socket reuse and connection limit
static CLIENT: Lazy<Client> = Lazy::new(|| Client {
    inner: reqwest::ClientBuilder::new()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .expect("build reqwest client"),
    semaphore: Semaphore::new(MAX_CONNECTIONS),
});

struct Client {
    inner: reqwest::Client,
    semaphore: Semaphore,
}

/// Fetch a resource at the provided [`Url`] and stream it's response bytes
pub async fn get(url: Url) -> Result<impl Stream<Item = Result<Bytes>>> {
    let permit_ = CLIENT
        .semaphore
        .acquire()
        .await
        .expect("aquire client permit");

    let response = CLIENT.inner.get(url).send().await?;

    response
        .error_for_status()
        .map(reqwest::Response::bytes_stream)
}
