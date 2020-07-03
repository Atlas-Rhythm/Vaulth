#[cfg(any(
    not(any(feature = "postgres", feature = "mysql")),
    all(feature = "postgres", feature = "mysql")
))]
compile_error!("A single database backend must be selected");

mod config;
mod hash;
mod providers;
mod token;
mod utils;

use anyhow::Result;
use config::Config;
use reqwest::Client as HttpClient;
use sqlx::Pool;
use std::{env, sync::Arc};
use warp::{Filter, Reply};

const LOG_ENV_VAR: &str = "VAULTH_LOG";

#[cfg(feature = "postgres")]
type DbConnection = sqlx::PgConnection;
#[cfg(feature = "mysql")]
type DbConnection = sqlx::MySqlConnection;

#[tokio::main]
async fn main() -> Result<()> {
    let config = config::read(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "vaulth.json".to_owned()),
    )
    .await?;
    let config = Arc::new(config);

    if env::var_os(LOG_ENV_VAR).is_none() {
        env::set_var(
            LOG_ENV_VAR,
            config.log_level.unwrap_or(log::Level::Info).to_string(),
        );
    }
    env_logger::init_from_env(LOG_ENV_VAR);

    log::debug!("creating database connection pool");
    let pool: Pool<DbConnection> = Pool::new(&config.database_url).await?;
    let pool = Arc::new(pool);

    log::debug!("creating http client");
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
    let client = Arc::new(client);

    serve(
        warp::any()
            .map(|| "Hello, World!")
            .with(warp::log("vaulth")),
        &config,
    )
    .await;

    Ok(())
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
