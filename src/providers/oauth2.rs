use crate::{
    config::{Config, OAuth2Config},
    providers::Params,
    token,
    utils::{self, TryExt},
    HttpClient,
};
use serde::{Deserialize, Serialize};
use std::{future::Future, sync::Arc};
use warp::{
    http::uri::{Parts, PathAndQuery, Uri},
    Filter, Rejection, Reply,
};

#[derive(Copy, Clone)]
pub struct ProviderInfo {
    pub name: &'static str,
    pub auth_uri: &'static str,
    pub token_uri: &'static str,
    pub scopes: &'static [&'static str],
}

pub fn handler<IdFnRet>(
    info: ProviderInfo,
    config: &'static OAuth2Config,
    global_config: &'static Config,
    http_client: &'static HttpClient,
    id_fn: fn(String, &'static HttpClient) -> IdFnRet,
) -> anyhow::Result<
    impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Sync + Clone + 'static,
>
where
    IdFnRet: Future<Output = anyhow::Result<String>> + Send + 'static,
{
    let auth_uri = Uri::from_static(info.auth_uri);
    let scopes = info.scopes.join(" ");

    let auth_uri = build_auth_uri(
        auth_uri.into_parts(),
        &config.client_id,
        &global_config.root_uri,
        info.name,
        &scopes,
    )?;
    let first_handler = warp::path::path(info.name)
        .and(warp::path::end())
        .and(warp::query::query())
        .and(utils::with_copied(global_config))
        .and_then(move |query: Params, global_config: &'static Config| {
            first_handler(query, global_config, auth_uri.clone())
        });

    let second_handler = warp::path::path(format!("{}-r", info.name))
        .and(warp::path::end())
        .and(warp::query::query())
        .and(utils::with_copied(global_config))
        .and(utils::with_copied(config))
        .and(utils::with_cloned(Arc::new(scopes)))
        .and(utils::with_copied(http_client))
        .and_then(
            move |query: RedirectParams,
                  global_config: &'static Config,
                  config: &'static OAuth2Config,
                  scopes: Arc<String>,
                  http_client: &'static HttpClient| {
                second_handler(
                    query,
                    global_config,
                    info.name,
                    config,
                    scopes,
                    http_client,
                    info.token_uri,
                    id_fn,
                )
            },
        );

    Ok(first_handler.or(second_handler))
}

#[tracing::instrument]
async fn first_handler(
    query: Params,
    global_config: &'static Config,
    auth_uri: Uri,
) -> Result<impl Reply, Rejection> {
    let state = token::encode(query, &global_config.token).await.or_ise()?;
    Ok(warp::redirect(
        finish_auth_uri(auth_uri.into_parts(), &state).or_ise()?,
    ))
}

#[tracing::instrument]
async fn second_handler<IdFnRet>(
    query: RedirectParams,
    global_config: &'static Config,
    name: &str,
    config: &'static OAuth2Config,
    scopes: Arc<String>,
    http_client: &'static HttpClient,
    token_uri: &str,
    id_fn: fn(String, &'static HttpClient) -> IdFnRet,
) -> Result<impl Reply, Rejection>
where
    IdFnRet: Future<Output = anyhow::Result<String>> + Send + 'static,
{
    let (code, state) = match query {
        RedirectParams::Success { code, state } => (code, state),
        RedirectParams::Error { error, state } => {
            let state: Params = token::decode(state, &global_config.token)
                .await
                .or_ise()?
                .or_ise()?;
            let uri =
                Uri::from_maybe_shared(error_redirect_uri_from_state(&state, &error)).or_ise()?;
            return Ok(warp::redirect::temporary(uri));
        }
    };
    let state: Params = token::decode(state, &global_config.token)
        .await
        .or_ise()?
        .or_ise()?;

    let redirect_uri = format!("{}/{}-r", &global_config.root_uri, name);
    let post_form = TokenRequest::new(
        &code,
        &config.client_id,
        &config.client_secret,
        &redirect_uri,
        &scopes,
    );

    let token = http_client
        .post(token_uri)
        .form(&post_form)
        .send()
        .await
        .or_ise()?
        .json::<TokenResponse>()
        .await
        .or_ise()?
        .access_token;

    let id = id_fn(token, http_client).await.or_ise()?;

    let token = token::encode(id, &global_config.token).await.or_ise()?;
    let uri = Uri::from_maybe_shared(success_redirect_uri_from_state(&state, &token)).or_ise()?;
    Ok(warp::redirect::temporary(uri))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RedirectParams {
    Success { code: String, state: String },
    Error { error: String, state: String },
}

#[derive(Serialize)]
struct TokenRequest<'a> {
    code: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    redirect_uri: &'a str,
    scope: &'a str,
    grant_type: &'static str,
}
impl<'a> TokenRequest<'a> {
    fn new(
        code: &'a str,
        client_id: &'a str,
        client_secret: &'a str,
        redirect_uri: &'a str,
        scope: &'a str,
    ) -> Self {
        Self {
            code,
            client_id,
            client_secret,
            redirect_uri,
            scope,
            grant_type: "authorization_code",
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

fn build_auth_uri(
    mut uri: Parts,
    client_id: &str,
    root_uri: &str,
    name: &str,
    scopes: &str,
) -> anyhow::Result<Uri> {
    let pq_start = match &uri.path_and_query {
        Some(pq) => format!("{}&", pq.as_str()),
        None => "?".to_owned(),
    };
    let pq = format!(
        "{}response_type=code&prompt=none&client_id={}&redirect_uri={}/{}-r&scope={}",
        pq_start, client_id, root_uri, name, scopes,
    );
    uri.path_and_query = Some(PathAndQuery::from_maybe_shared(pq.as_str().to_owned())?);
    Ok(Uri::from_parts(uri)?)
}

fn finish_auth_uri(mut uri: Parts, state: &str) -> anyhow::Result<Uri> {
    let pq = format!("{}&state={}", uri.path_and_query.as_ref().unwrap(), state);
    uri.path_and_query = Some(PathAndQuery::from_maybe_shared(pq.as_str().to_owned())?);
    Ok(Uri::from_parts(uri)?)
}

fn error_redirect_uri_from_state(params: &Params, error: &str) -> String {
    match &params.state {
        Some(s) => format!("{}?error={}&state={}", params.redirect_uri, error, s),
        None => format!("{}?error={}", params.redirect_uri, error),
    }
}

fn success_redirect_uri_from_state(params: &Params, token: &str) -> String {
    match &params.state {
        Some(s) => format!("{}?token={}&state={}", params.redirect_uri, token, s),
        None => format!("{}?token={}", params.redirect_uri, token),
    }
}
