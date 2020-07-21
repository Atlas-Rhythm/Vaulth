use crate::providers::oauth::{self, ProviderInfo, SharedResources};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

const NAME: &str = "github";

fn redirect_uri(root: &str) -> String {
    format!("{}/{}-r", root, NAME)
}

fn uri_fn(client_id: &str, root: &str, state: &str) -> String {
    format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&state={}",
        client_id,
        redirect_uri(root),
        state,
    )
}

async fn id_fn(code: String, state: String, shared: SharedResources) -> Result<String> {
    #[derive(Serialize)]
    struct TokenRequest<'a> {
        client_id: &'static str,
        client_secret: &'static str,
        code: &'a str,
        redirect_uri: &'a str,
        state: &'a str,
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    #[derive(Deserialize)]
    struct UserResponse {
        id: i32,
    }

    let config = shared.config.context("unsupported provider")?;

    let token = shared
        .http_client
        .post("https://github.com/login/oauth/access_token")
        .json(&TokenRequest {
            client_id: &config.client_id,
            client_secret: &config.client_secret,
            code: &code,
            redirect_uri: &redirect_uri(&shared.global_config.root_uri),
            state: &state,
        })
        .send()
        .await?
        .json::<TokenResponse>()
        .await?
        .access_token;

    let id = shared
        .http_client
        .get("https://api.github.com/user")
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?
        .json::<UserResponse>()
        .await?
        .id
        .to_string();

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
