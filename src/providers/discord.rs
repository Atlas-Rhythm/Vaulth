use crate::{
    config::{Config, OAuth2Config},
    HttpClient,
};
use anyhow::Result;
use serde::Deserialize;
use std::sync::Arc;
use warp::{http::Uri, Filter, Rejection, Reply};

const NAME: &str = "discord";
const AUTH_URI: &str = "https://discord.com/api/oauth2/authorize";
const TOKEN_URI: &str = "https://discord.com/api/oauth2/token";
const SCOPES: &[&str] = &["identify"];

#[derive(Deserialize)]
struct UserResponse {
    id: String,
}

async fn id_fn(token: String, http_client: Arc<HttpClient>) -> Result<String> {
    Ok(http_client
        .get("https://discord.com/api/v6/users/@me")
        .bearer_auth(token)
        .send()
        .await?
        .json::<UserResponse>()
        .await?
        .id)
}

pub fn handler(
    config: Arc<OAuth2Config>,
    global_config: Arc<Config>,
    http_client: Arc<HttpClient>,
) -> Result<impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Sync + Clone + 'static>
{
    let auth_uri = Uri::from_maybe_shared(AUTH_URI)?;
    super::oauth2::handler(
        NAME,
        auth_uri,
        TOKEN_URI,
        SCOPES,
        config,
        global_config,
        http_client,
        id_fn,
    )
}
