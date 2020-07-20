use crate::{
    config::{Config, OAuth2Config},
    db::User,
    providers::{CodeJwt, Params},
    token,
    utils::{self, TryExt},
    DbConnection, HttpClient,
};
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use sqlx::Pool;
use std::{future::Future, sync::Arc};
use warp::{
    http::uri::{Parts, PathAndQuery, Uri},
    Filter, Rejection, Reply,
};

/// Information about a standard OAuth2 provider
#[derive(Copy, Clone)]
pub struct ProviderInfo {
    pub name: &'static str,
    pub auth_uri: &'static str,
    pub token_uri: &'static str,
    pub scopes: &'static [&'static str],
}

/// Resources shared across handlers
#[derive(Copy, Clone, Derivative)]
#[derivative(Debug)]
pub struct SharedResources {
    pub config: &'static OAuth2Config,
    pub global_config: &'static Config,
    #[derivative(Debug = "ignore")]
    pub http_client: &'static HttpClient,
    #[derivative(Debug = "ignore")]
    pub pool: &'static Pool<DbConnection>,
}

/// Generates a filter handling everything for a single provider
pub fn handler<IdFnRet>(
    info: ProviderInfo,
    shared: SharedResources,
    id_fn: fn(String, &'static HttpClient) -> IdFnRet,
) -> anyhow::Result<
    impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Sync + Clone + 'static,
>
where
    IdFnRet: Future<Output = anyhow::Result<String>> + Send + 'static,
{
    tracing::debug!("generating {} handlers", info.name);

    let auth_uri = Uri::from_static(info.auth_uri);
    let scopes = info.scopes.join(" ");

    // Create the provider URI where the user is first redirected
    let auth_uri = build_auth_uri(
        auth_uri.into_parts(),
        &shared.config.client_id,
        &shared.global_config.root_uri,
        info.name,
        &scopes,
    )?;

    let first_handler = warp::path::path(info.name)
        .and(warp::path::end())
        .and(warp::query::query())
        .and(utils::with_copied(shared.global_config))
        .and_then(move |query: Params, global_config: &'static Config| {
            first_handler(query, global_config, auth_uri.clone())
        });

    let second_handler = warp::path::path(format!("{}-r", info.name))
        .and(warp::path::end())
        .and(warp::query::query())
        .and(utils::with_copied(shared))
        .and(utils::with_cloned(Arc::new(scopes)))
        .and_then(
            move |query: RedirectParams, shared: SharedResources, scopes: Arc<String>| {
                second_handler(query, info.name, scopes, info.token_uri, shared, id_fn)
            },
        );

    Ok(first_handler.or(second_handler))
}

/// This is where the user is redirected by the client
/// The handler translates and stores important info, then redirects the user to the provider
#[tracing::instrument]
async fn first_handler(
    query: Params,
    global_config: &'static Config,
    auth_uri: Uri,
) -> Result<impl Reply, Rejection> {
    // Verify the infos are valid
    match global_config.clients.get(&query.client_id) {
        Some(c) => {
            if !c.redirect_urls.iter().any(|u| u == &query.redirect_uri) {
                let uri = Uri::from_maybe_shared(error_redirect_uri_from_state(
                    &query,
                    "invalid redirect_uri",
                ))
                .or_ise()?;
                return Ok(warp::redirect::temporary(uri));
            }
        }
        None => {
            let uri =
                Uri::from_maybe_shared(error_redirect_uri_from_state(&query, "invalid client_id"))
                    .or_ise()?;
            return Ok(warp::redirect::temporary(uri));
        }
    };

    // Encode the client id and redirect url in the state that will be sent to the provider
    // Required to know where to forward info from the provider
    // Using a JWT for the task makes it possible to store state and provide security at the same time
    let state = token::encode(query, &global_config.token).await.or_ise()?;
    Ok(warp::redirect::temporary(
        finish_auth_uri(auth_uri.into_parts(), &state).or_ise()?,
    ))
}

/// This is where the user is redirected from the provider, and where the heavy lifting is done
/// The handler exchanges the code for a token, then uses it to obtain the provider user ID,
/// then creates a code that can be exchanged by the client for a token for the Vaulth user linked to the provider user ID,
/// and finally redirects the user back to the client
#[tracing::instrument]
async fn second_handler<IdFnRet>(
    query: RedirectParams,
    name: &str,
    scopes: Arc<String>,
    token_uri: &str,
    shared: SharedResources,
    id_fn: fn(String, &'static HttpClient) -> IdFnRet,
) -> Result<impl Reply, Rejection>
where
    IdFnRet: Future<Output = anyhow::Result<String>> + Send + 'static,
{
    // Try to extract the initial query params from the state returned by the provider
    let (code, state) = match query {
        RedirectParams::Success { code, state } => (code, state),
        RedirectParams::Error { error, state } => {
            // It's ok to not forward the error here cause it can only be cause by malicious requests
            let state: Params = token::decode(state, &shared.global_config.token)
                .await
                .or_ise()?
                .or_ise()?;
            let uri =
                Uri::from_maybe_shared(error_redirect_uri_from_state(&state, &error)).or_ise()?;
            return Ok(warp::redirect::temporary(uri));
        }
    };
    let state: Params = token::decode(state, &shared.global_config.token)
        .await
        .or_ise()?
        .or_ise()?;

    // Create the form used to exchange a code for a token
    let redirect_uri = format!("{}/{}-r", &shared.global_config.root_uri, name);
    let post_form = TokenRequest::new(
        &code,
        &shared.config.client_id,
        &shared.config.client_secret,
        &redirect_uri,
        &scopes,
    );

    // Exchange the code for a token
    let token = shared
        .http_client
        .post(token_uri)
        .form(&post_form)
        .send()
        .await
        .or_ise()?
        .json::<TokenResponse>()
        .await
        .or_ise()?
        .access_token;

    // Defer to the provider-specific code to grab an ID from the token
    let provider_id = id_fn(token, shared.http_client).await.or_ise()?;

    // Try to find a Vaulth user matching that provider ID
    let user_id = User::select_by_provider(name, &provider_id, shared.pool)
        .await
        .or_ise()?;

    // Generate a code the client can exchange for a Vaulth token
    let code = CodeJwt {
        provider_name: name.to_owned(),
        provider_id,
        client_id: state.client_id.clone(),
    };
    let code = token::encode(code, &shared.global_config.token)
        .await
        .or_ise()?;

    // Redirect the user back to the client
    let uri = Uri::from_maybe_shared(success_redirect_uri_from_state(
        &state,
        &code,
        user_id.as_ref().map(AsRef::as_ref),
    ))
    .or_ise()?;
    Ok(warp::redirect::temporary(uri))
}

/// Redirect query parameters from a standard OAuth2 provider
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RedirectParams {
    Success { code: String, state: String },
    Error { error: String, state: String },
}

/// Standard OAuth2 code exchange request body
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

/// Builds part of a standard OAuth2 authentication URI (used for providers)
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

/// Adds the state and finishes a standard OAuth2 authentication URI (used for providers)
fn finish_auth_uri(mut uri: Parts, state: &str) -> anyhow::Result<Uri> {
    let pq = format!("{}&state={}", uri.path_and_query.as_ref().unwrap(), state);
    uri.path_and_query = Some(PathAndQuery::from_maybe_shared(pq.as_str().to_owned())?);
    Ok(Uri::from_parts(uri)?)
}

/// Creates a redirect URI for errors (used for the client)
fn error_redirect_uri_from_state(params: &Params, error: &str) -> String {
    match &params.state {
        Some(s) => format!("{}?error={}&state={}", params.redirect_uri, error, s),
        None => format!("{}?error={}", params.redirect_uri, error),
    }
}

/// Creates a redirect URI for success (used for the client)
fn success_redirect_uri_from_state(params: &Params, code: &str, user: Option<&str>) -> String {
    let mut uri = format!("{}?code={}", params.redirect_uri, code);
    if let Some(state) = &params.state {
        uri = format!("{}&state={}", uri, state);
    }
    if let Some(user) = user {
        uri = format!("{}&user={}", uri, user);
    }
    uri
}
