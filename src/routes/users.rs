use crate::{
    config::Config,
    db::User,
    errors::{JsonError, TryExt},
    jwt,
    providers::TokenJwt,
    DbConnection,
};
use sqlx::Pool;
use warp::{http::StatusCode, Filter, Rejection, Reply};

pub fn handler(
    config: &'static Config,
    pool: &'static Pool<DbConnection>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static {
    let user = warp::path!("users" / String).and_then(move |id: String| user(id, pool));
    let me = warp::path!("me")
        .and(warp::header("Authorization"))
        .and_then(move |auth: String| me(auth, config, pool));
    (user).or(me)
}

async fn user(id: String, pool: &'static Pool<DbConnection>) -> Result<impl Reply, Rejection> {
    let user = User::select(&id, pool).await.or_ise()?.or_nf()?;
    Ok(warp::reply::json(&user))
}

async fn me(
    mut auth: String,
    config: &'static Config,
    pool: &'static Pool<DbConnection>,
) -> Result<impl Reply, Rejection> {
    if !auth.starts_with("Bearer ") {
        None.or_json(
            JsonError {
                error: "invalid authorization header",
            },
            StatusCode::BAD_REQUEST,
        )?;
    }
    auth.replace_range(0..7, "");

    let token: TokenJwt = jwt::decode(auth, &config.token).await.or_ise()?.or_json(
        JsonError {
            error: "invalid token",
        },
        StatusCode::UNAUTHORIZED,
    )?;

    let user = User::select(&token.sub, pool).await.or_ise()?.or_nf()?;
    Ok(warp::reply::json(&user))
}
