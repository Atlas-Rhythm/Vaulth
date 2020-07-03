use crate::{config::Config, providers::State, token, utils::TryExt, DbConnection, HttpClient};
use serde::{Deserialize, Serialize};
use sqlx::Pool;
use std::sync::Arc;
use warp::{http::Uri, Rejection, Reply};

pub async fn redirect(params: State, config: Arc<Config>) -> Result<impl Reply, Rejection> {
    let state = token::encode(params, &config.token)
        .await
        .or_internal_server_error()?;

    let uri = Uri::builder().scheme("https").authority("accounts.google.com").path_and_query(format!(
        "/o/oauth2/v2/auth?client_id={}&redirect_uri={}/google-r&response_type=code&scope=profile openid&access_type=online&state={}",
        &config.google.as_ref().or_internal_server_error()?.client_id, &config.root_uri, state,
    ).as_str()).build().or_internal_server_error()?;
    Ok(warp::redirect::temporary(uri))
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RedirectedParams {
    Success { code: String, state: String },
    Error { error: String, state: String },
}

#[derive(Serialize)]
struct TokenRequest<'a> {
    code: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    redirect_uri: &'a str,
    grant_type: &'static str,
}
impl<'a> TokenRequest<'a> {
    fn new(
        code: &'a str,
        client_id: &'a str,
        client_secret: &'a str,
        redirect_uri: &'a str,
    ) -> Self {
        Self {
            code,
            client_id,
            client_secret,
            redirect_uri,
            grant_type: "authorization_code",
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}

pub async fn redirected(
    params: RedirectedParams,
    config: Arc<Config>,
    http_client: Arc<HttpClient>,
    db_pool: Arc<Pool<DbConnection>>,
) -> Result<impl Reply, Rejection> {
    let (code, state) = match params {
        RedirectedParams::Success { code, state } => (code, state),
        RedirectedParams::Error { error, state } => {
            let state: State = token::decode(state, &config.token)
                .await
                .or_internal_server_error()?
                .or_internal_server_error()?;
            let uri = Uri::from_maybe_shared(redirect_uri_from_state(&state))
                .or_internal_server_error()?;
            return Ok(warp::redirect::temporary(uri));
        }
    };

    let redirect_uri = format!("{}/google-r", &config.root_uri);
    let post_form = TokenRequest::new(
        &code,
        &config.google.as_ref().or_internal_server_error()?.client_id,
        &config
            .google
            .as_ref()
            .or_internal_server_error()?
            .client_secret,
        &redirect_uri,
    );

    let token = http_client
        .post("https://oauth2.googleapis.com/token")
        .form(&post_form)
        .send()
        .await
        .or_internal_server_error()?
        .json::<TokenResponse>()
        .await
        .or_internal_server_error()?
        .id_token;

    Ok(warp::redirect::temporary(""))
}

fn redirect_uri_from_state(state: &State) -> String {
    match &state.state {
        Some(s) => format!("{}?state={}", state.redirect_uri, s),
        None => state.redirect_uri.to_owned(),
    }
}
