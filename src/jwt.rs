use crate::config::TokenConfig;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{errors::ErrorKind, Algorithm, DecodingKey, EncodingKey, Header};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt;
use tokio::{fs, task};

#[derive(Serialize, Deserialize)]
struct Claims<T> {
    #[serde(with = "chrono_jwt")]
    exp: DateTime<Utc>,
    #[serde(with = "chrono_jwt")]
    iat: DateTime<Utc>,

    #[serde(flatten)]
    data: T,
}

/// Encodes and returns a JWT for the specified user
#[tracing::instrument]
pub async fn encode<T>(data: T, config: &TokenConfig) -> Result<String>
where
    T: Send + Serialize + fmt::Debug + 'static,
{
    tracing::debug!("encoding jwt token");

    let duration = Duration::minutes(config.duration);
    let key = fs::read(&config.private_key).await?;
    Ok(task::spawn_blocking(move || encode_sync(data, duration, key)).await??)
}
fn encode_sync<T>(data: T, duration: Duration, key: Vec<u8>) -> Result<String>
where
    T: Serialize,
{
    let now = Utc::now();
    Ok(jsonwebtoken::encode(
        &Header::new(Algorithm::ES384),
        &Claims {
            exp: now + duration,
            iat: now,
            data,
        },
        &EncodingKey::from_ec_pem(&key)?,
    )?)
}

/// Decodes a JWT and returns the user it refers to if valid
#[tracing::instrument]
pub async fn decode<T>(token: String, config: &TokenConfig) -> Result<Option<T>>
where
    T: Send + DeserializeOwned + fmt::Debug + 'static,
{
    tracing::debug!("decoding jwt token");

    let key = fs::read(&config.public_key).await?;
    Ok(task::spawn_blocking(move || decode_sync(token, key)).await??)
}
fn decode_sync<T>(token: String, key: Vec<u8>) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    match jsonwebtoken::decode::<Claims<T>>(
        &token,
        &DecodingKey::from_secret(&key),
        &Default::default(),
    ) {
        Ok(data) => Ok(Some(data.claims.data)),
        Err(e) => match e.kind() {
            ErrorKind::InvalidKeyFormat | ErrorKind::Crypto(_) => Err(e.into()),
            _ => Ok(None),
        },
    }
}

mod chrono_jwt {
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let timestamp = date.timestamp();
        serializer.serialize_i64(timestamp)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Utc.timestamp_opt(i64::deserialize(deserializer)?, 0)
            .single()
            .ok_or_else(|| serde::de::Error::custom("invalid Unix timestamp"))
    }
}
