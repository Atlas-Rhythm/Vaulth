use crate::providers::oauth::{self, ProviderInfo, SharedResources};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

const NAME: &str = "discord";

fn redirect_uri(root: &str) -> String {
    format!("{}/{}-r", root, NAME)
}

fn uri_fn(shared: SharedResources) -> String {
    format!(
        "https://discord.com/api/oauth2/authorize?response_type=code&scope=identify&prompt=none&client_id={}&redirect_uri={}",
        shared.config.client_id, redirect_uri(&shared.global_config.root_uri),
    )
}

async fn id_fn(code: String, shared: SharedResources) -> Result<String> {
    #[derive(Serialize)]
    struct TokenRequest<'a> {
        client_id: &'static str,
        client_secret: &'static str,
        grant_type: &'static str,
        code: &'a str,
        redirect_uri: &'a str,
        scope: &'static str,
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    #[derive(Deserialize)]
    struct UserResponse {
        id: String,
    }

    let token = shared
        .http_client
        .post("https://discord.com/api/v6/oauth2/token")
        .json(&TokenRequest {
            client_id: &shared.config.client_id,
            client_secret: &shared.config.client_secret,
            grant_type: "authorization_code",
            code: &code,
            redirect_uri: &redirect_uri(&shared.global_config.root_uri),
            scope: "identity",
        })
        .send()
        .await?
        .json::<TokenResponse>()
        .await?
        .access_token;

    let id = shared
        .http_client
        .get("https://discord.com/api/v6/users/@me")
        .bearer_auth(token)
        .send()
        .await?
        .json::<UserResponse>()
        .await?
        .id;

    Ok(id)
}

pub fn handler(
    shared: SharedResources,
) -> Result<impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Sync + Clone + 'static>
{
    oauth::handler(
        ProviderInfo {
            name: NAME,
            uri_fn,
            id_fn,
        },
        shared,
    )
}
