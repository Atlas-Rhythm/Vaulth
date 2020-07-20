use super::oauth2::{self, ProviderInfo};
use crate::{
    config::{Config, OAuth2Config},
    HttpClient,
};
use anyhow::Result;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

const INFO: ProviderInfo = ProviderInfo {
    name: "discord",
    auth_uri: "https://discord.com/api/oauth2/authorize",
    token_uri: "https://discord.com/api/oauth2/token",
    scopes: &["identify"],
};

#[derive(Deserialize)]
struct UserResponse {
    id: String,
}

async fn id_fn(token: String, http_client: &'static HttpClient) -> Result<String> {
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
    config: &'static OAuth2Config,
    global_config: &'static Config,
    http_client: &'static HttpClient,
) -> Result<impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Sync + Clone + 'static>
{
    oauth2::handler(INFO, config, global_config, http_client, id_fn)
}
