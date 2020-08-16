use crate::{
    config::Config,
    db::User,
    errors::{JsonError, TryExt},
    jwt,
    providers::TokenJwt,
};
use sqlx::PgPool;
use warp::{http::StatusCode, Filter, Rejection, Reply};

#[tracing::instrument(level = "debug")]
pub fn handler(
    config: &'static Config,
    pool: &'static PgPool,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static {
    let user = warp::path!("users" / String).and_then(move |id: String| user(id, pool));
    let me = warp::path!("me")
        .and(warp::header("Authorization"))
        .and_then(move |auth: String| me(auth, config, pool));
    (user).or(me)
}

#[tracing::instrument(level = "debug")]
async fn user(id: String, pool: &'static PgPool) -> Result<impl Reply, Rejection> {
    let user = User::select(&id, pool).await.or_ise()?.or_nf()?;
    Ok(warp::reply::json(&user))
}

#[tracing::instrument(level = "debug")]
async fn me(
    auth: String,
    config: &'static Config,
    pool: &'static PgPool,
) -> Result<impl Reply, Rejection> {
    if !auth.starts_with("Bearer ") {
        None.or_json(
            JsonError {
                error: "invalid authorization header",
            },
            StatusCode::BAD_REQUEST,
        )?;
    }

    let token: TokenJwt = jwt::decode(auth[7..].to_owned(), &config.token)
        .await
        .or_ise()?
        .or_json(
            JsonError {
                error: "invalid token",
            },
            StatusCode::UNAUTHORIZED,
        )?;

    let user = User::select(&token.sub, pool).await.or_ise()?.or_nf()?;
    Ok(warp::reply::json(&user))
}
