use crate::{
    config::{Config, OAuth2Config},
    db::User,
    errors::TryExt,
    jwt,
    providers::{CodeJwt, Params},
    DbConnection, HttpClient,
};
use derivative::Derivative;
use serde::Deserialize;
use sqlx::Pool;
use std::future::Future;
use warp::{http::uri::Uri, Filter, Rejection, Reply};

/// Information about a standard OAuth2 provider
#[derive(Derivative)]
#[derivative(Debug)]
pub struct ProviderInfo<IdFnRet> {
    /// Name of the provider
    pub name: &'static str,
    /// Function used to start building the auth URI
    #[derivative(Debug = "ignore")]
    pub uri_fn: fn(SharedResources) -> String,
    /// Function used to obtain an ID from a provider
    #[derivative(Debug = "ignore")]
    pub id_fn: fn(String, Params, SharedResources) -> IdFnRet,
}

impl<IdFnRet> Copy for ProviderInfo<IdFnRet> {}
impl<IdFnRet> Clone for ProviderInfo<IdFnRet> {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            uri_fn: self.uri_fn,
            id_fn: self.id_fn,
        }
    }
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
    provider: ProviderInfo<IdFnRet>,
    shared: SharedResources,
) -> anyhow::Result<
    impl Filter<Extract = (impl Reply,), Error = Rejection> + Send + Sync + Clone + 'static,
>
where
    IdFnRet: Future<Output = anyhow::Result<String>> + Send + 'static,
{
    tracing::debug!("generating {} handlers", provider.name);

    let uri = (provider.uri_fn)(shared);

    let first_handler = warp::path::path(provider.name)
        .and(warp::path::end())
        .and(warp::query::query())
        .and_then(move |query: Params| first_handler(query, shared.global_config, uri.clone()));

    let second_handler = warp::path::path(format!("{}-r", provider.name))
        .and(warp::path::end())
        .and(warp::query::query())
        .and_then(move |query: RedirectParams| second_handler(query, provider, shared));

    Ok(first_handler.or(second_handler))
}

/// This is where the user is redirected by the client
/// The handler translates and stores important info, then redirects the user to the provider
#[tracing::instrument]
async fn first_handler(
    query: Params,
    global_config: &'static Config,
    uri: String,
) -> Result<impl Reply, Rejection> {
    // Verify the infos are valid
    let client = global_config
        .clients
        .get(&query.client_id)
        .or_redirect("invalid client_id", &query)?;
    client
        .redirect_urls
        .iter()
        .find(|u| query.redirect_uri.starts_with(*u))
        .or_redirect("invalid redirect_uri", &query)?;

    // Encode the client id and redirect url in the state that will be sent to the provider
    // Required to know where to forward info from the provider
    // Using a JWT for the task makes it possible to store state and provide security at the same time
    let state = jwt::encode(query, &global_config.token).await.or_ise()?;
    Ok(warp::redirect::temporary(
        finish_auth_uri(&uri, &state).or_ise()?,
    ))
}

/// Redirect query parameters from a standard OAuth2 provider
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RedirectParams {
    Success { code: String, state: String },
    Error { error: String, state: String },
}

/// This is where the user is redirected from the provider, and where the heavy lifting is done
/// The handler exchanges the code for a token, then uses it to obtain the provider user ID,
/// then creates a code that can be exchanged by the client for a token for the Vaulth user linked to the provider user ID,
/// and finally redirects the user back to the client
#[tracing::instrument]
async fn second_handler<IdFnRet>(
    query: RedirectParams,
    provider: ProviderInfo<IdFnRet>,
    shared: SharedResources,
) -> Result<impl Reply, Rejection>
where
    IdFnRet: Future<Output = anyhow::Result<String>> + Send + 'static,
{
    // Try to extract the initial query params from the state returned by the provider
    let (code, state) = match query {
        RedirectParams::Success { code, state } => (code, state),
        RedirectParams::Error { error, state } => {
            // It's ok to not forward the error here cause it can only be cause by malicious requests
            let params: Params = jwt::decode(state, &shared.global_config.token)
                .await
                .or_ise()?
                .or_ise()?;
            let uri = Uri::from_maybe_shared(match &params.state {
                Some(s) => format!("{}?error={}&state={}", params.redirect_uri, error, s),
                None => format!("{}?error={}", params.redirect_uri, error),
            })
            .or_ise()?;
            return Ok(warp::redirect::temporary(uri));
        }
    };
    // It's ok to not forward the error here cause it can only be cause by malicious requests
    let params: Params = jwt::decode(state, &shared.global_config.token)
        .await
        .or_ise()?
        .or_ise()?;

    // Defer to the provider-specific code to grab an ID using the code
    let provider_id = (provider.id_fn)(code, params.clone(), shared)
        .await
        .or_redirect("couldn't obtain id from provider", &params)?;

    // Try to find a Vaulth user matching that provider ID
    let user_id = User::select_by_provider(provider.name, &provider_id, shared.pool)
        .await
        .or_redirect("internal server error", &params)?;

    // Generate a code the client can exchange for a Vaulth token
    let code = CodeJwt {
        provider_name: provider.name.to_owned(),
        provider_id,
        client_id: params.client_id.clone(),
    };
    let code = jwt::encode(code, &shared.global_config.token)
        .await
        .or_redirect("internal server error", &params)?;

    // Redirect the user back to the client
    let uri = Uri::from_maybe_shared(success_redirect_uri_from_state(
        &params,
        &code,
        user_id.as_ref().map(AsRef::as_ref),
    ))
    .or_redirect("internal server error", &params)?;
    Ok(warp::redirect::temporary(uri))
}

/// Adds the state and finishes a standard OAuth2 authentication URI (used for providers)
fn finish_auth_uri(uri: &str, state: &str) -> anyhow::Result<Uri> {
    let uri = format!("{}&state={}", uri, state);
    Ok(Uri::from_maybe_shared(uri)?)
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
