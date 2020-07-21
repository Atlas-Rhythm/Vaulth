#[cfg(any(
    not(any(feature = "postgres", feature = "mysql")),
    all(feature = "postgres", feature = "mysql")
))]
compile_error!("A single database backend must be selected");

mod config;
mod db;
mod errors;
mod jwt;
mod password;
mod providers;
mod routes;

use anyhow::Result;
use config::Config;
use providers::oauth::SharedResources;
use reqwest::Client as HttpClient;
use routes::token::TokenRequestBody;
use sqlx::Pool;
use std::env;
use tracing_subscriber::EnvFilter;
use warp::{Filter, Reply};

const LOG_ENV_VAR: &str = "VAULTH_LOG";

#[cfg(feature = "postgres")]
type DbConnection = sqlx::PgConnection;
#[cfg(feature = "mysql")]
type DbConnection = sqlx::MySqlConnection;

#[tokio::main]
async fn main() -> Result<()> {
    let config = config().await?;

    if env::var_os(LOG_ENV_VAR).is_none() {
        env::set_var(
            LOG_ENV_VAR,
            config
                .log_level
                .as_ref()
                .unwrap_or(&tracing::Level::INFO.to_string()),
        );
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env(LOG_ENV_VAR))
        .init();

    let pool = pool(config).await?;
    let client = client(&config).await?;

    let shared = SharedResources {
        config: config.discord.as_ref().unwrap(),
        global_config: config,
        http_client: client,
        pool,
    };

    let routes = (providers::discord::handler(SharedResources {
        config: config.discord.as_ref().unwrap(),
        ..shared
    })?)
    .or(providers::github::handler(SharedResources {
        config: config.github.as_ref().unwrap(),
        ..shared
    })?)
    .or(warp::path!("token")
        .and(warp::body::json())
        .and_then(move |body: TokenRequestBody| routes::token::token(body, config, pool)))
    .or(warp::path!("token" / String)
        .and(warp::body::json())
        .and_then(move |user: String, body: TokenRequestBody| {
            routes::token::token_user(user, body, config, pool)
        }));

    serve(
        routes
            .recover(errors::handle_redirects)
            .recover(errors::handle_json),
        &config,
    )
    .await;
    Ok(())
}

async fn config() -> Result<&'static Config> {
    let config = config::read(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "vaulth.json".to_owned()),
    )
    .await?;
    Ok(Box::leak(Box::new(config)))
}

#[tracing::instrument]
async fn pool(config: &Config) -> Result<&'static Pool<DbConnection>> {
    tracing::debug!("creating database connection pool");

    let pool: Pool<DbConnection> = Pool::new(&config.database_url).await?;
    Ok(&*Box::leak(Box::new(pool)))
}

#[tracing::instrument]
async fn client(config: &Config) -> Result<&'static HttpClient> {
    tracing::debug!("creating http client");

    let client = HttpClient::builder()
        .user_agent(
            config
                .user_agent
                .as_ref()
                .map(AsRef::as_ref)
                .unwrap_or(concat!(
                    env!("CARGO_PKG_NAME"),
                    "/",
                    env!("CARGO_PKG_VERSION")
                )),
        )
        .build()?;
    Ok(&*Box::leak(Box::new(client)))
}

async fn serve(
    filter: impl Filter<Extract = (impl Reply,)> + Send + Sync + Clone + 'static,
    config: &Config,
) {
    if let Some(tls_config) = &config.tls {
        warp::serve(filter)
            .tls()
            .cert_path(&tls_config.cert)
            .key_path(&tls_config.key)
            .run(([127, 0, 0, 1], config.port))
            .await
    } else {
        warp::serve(filter).run(([127, 0, 0, 1], config.port)).await
    }
}
