use crate::config::TokenConfig;
use crate::errors::TryExt;
use tokio::fs;
use warp::{Filter, Rejection, Reply};

pub fn handler(
    config: &'static TokenConfig,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static {
    warp::path!("key").and_then(move || key(config))
}

async fn key(config: &'static TokenConfig) -> Result<impl Reply, Rejection> {
    let contents = fs::read(&config.public_key).await.or_ise()?;
    Ok(warp::reply::with_header(
        contents,
        "Content-Type",
        "application/x-pem-file",
    ))
}
