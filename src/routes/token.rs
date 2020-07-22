use crate::{
    config::Config,
    db::User,
    errors::{JsonError, TryExt},
    jwt,
    providers::{CodeJwt, TokenJwt},
    DbConnection,
};
use serde::{Deserialize, Serialize};
use sqlx::Pool;
use warp::{http::StatusCode, Filter, Rejection, Reply};

#[derive(Debug, Deserialize)]
pub struct TokenRequestBody {
    client_id: String,
    client_secret: String,
    code: String,
}

#[derive(Debug, Serialize)]
struct SuccessResponse {
    access_token: String,
    expires_in: i64,
}

pub fn handler(
    config: &'static Config,
    pool: &'static Pool<DbConnection>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static {
    let token = warp::path!("token")
        .and(warp::body::json())
        .and_then(move |body: TokenRequestBody| token(body, config, pool));
    let token_user = warp::path!("token" / String)
        .and(warp::body::json())
        .and_then(move |user: String, body: TokenRequestBody| token_user(user, body, config, pool));
    (token).or(token_user)
}

#[tracing::instrument(skip(pool))]
async fn token(
    body: TokenRequestBody,
    config: &'static Config,
    pool: &'static Pool<DbConnection>,
) -> Result<impl Reply, Rejection> {
    let code = verify(body, config).await?;
    let user: String = User::select_by_provider(&code.provider_name, &code.provider_id, pool)
        .await
        .or_ise()?
        .or_json(
            JsonError {
                error: "no matching user",
            },
            StatusCode::BAD_REQUEST,
        )?;

    User::login(&user, pool).await.or_ise()?;

    let token = jwt::encode(TokenJwt { sub: user }, &config.token)
        .await
        .or_ise()?;
    Ok(warp::reply::json(&SuccessResponse {
        access_token: token,
        expires_in: config.token.duration,
    }))
}

#[tracing::instrument(skip(pool))]
async fn token_user(
    given_user: String,
    body: TokenRequestBody,
    config: &'static Config,
    pool: &'static Pool<DbConnection>,
) -> Result<impl Reply, Rejection> {
    let code = verify(body, config).await?;
    let user: Option<String> =
        User::select_by_provider(&code.provider_name, &code.provider_id, pool)
            .await
            .or_ise()?;

    if let Some(user) = user {
        if user != given_user {
            None.or_json(
                JsonError {
                    error: "mismatched users",
                },
                StatusCode::BAD_REQUEST,
            )?;
        }

        User::login(&user, pool).await.or_ise()?;

        let token = jwt::encode(TokenJwt { sub: user }, &config.token)
            .await
            .or_ise()?;
        return Ok(warp::reply::with_status(
            warp::reply::json(&SuccessResponse {
                access_token: token,
                expires_in: config.token.duration,
            }),
            StatusCode::OK,
        ));
    }

    User::register_by_provider(&given_user, &code.provider_name, &code.provider_id, pool)
        .await
        .or_ise()?;

    let token = jwt::encode(TokenJwt { sub: given_user }, &config.token)
        .await
        .or_ise()?;
    Ok(warp::reply::with_status(
        warp::reply::json(&SuccessResponse {
            access_token: token,
            expires_in: config.token.duration,
        }),
        StatusCode::CREATED,
    ))
}

async fn verify(body: TokenRequestBody, config: &'static Config) -> Result<CodeJwt, Rejection> {
    let code: CodeJwt = jwt::decode(body.code, &config.token)
        .await
        .or_ise()?
        .or_json(
            JsonError {
                error: "invalid code",
            },
            StatusCode::BAD_REQUEST,
        )?;

    let client = config.clients.get(&code.client_id).or_json(
        JsonError {
            error: "invalid code",
        },
        StatusCode::BAD_REQUEST,
    )?;

    if body.client_secret != client.client_secret {
        None.or_json(
            JsonError {
                error: "invalid client_secret",
            },
            StatusCode::BAD_REQUEST,
        )?;
    }

    Ok(code)
}
