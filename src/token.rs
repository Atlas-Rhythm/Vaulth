use crate::config::TokenConfig;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{errors::ErrorKind, DecodingKey, EncodingKey};
use serde::{Deserialize, Serialize};
use tokio::task;

#[derive(Serialize, Deserialize)]
struct Claims {
    #[serde(with = "chrono_jwt")]
    exp: DateTime<Utc>,
    #[serde(with = "chrono_jwt")]
    iat: DateTime<Utc>,
    sub: String,
}

/// Encodes and returns a JWT for the specified user
pub async fn encode(user: String, config: &TokenConfig) -> Result<String> {
    log::debug!("encoding jwt token");

    let duration = Duration::minutes(config.duration);
    let key = EncodingKey::from_secret(config.key.as_bytes());
    Ok(task::spawn_blocking(move || encode_sync(user, duration, key)).await??)
}
fn encode_sync(sub: String, duration: Duration, key: EncodingKey) -> Result<String> {
    let now = Utc::now();
    Ok(jsonwebtoken::encode(
        &Default::default(),
        &Claims {
            exp: now + duration,
            iat: now,
            sub,
        },
        &key,
    )?)
}

/// Decodes a JWT and returns the user it refers to if valid
pub async fn decode(token: String, config: &TokenConfig) -> Result<Option<String>> {
    log::debug!("decoding jwt token");

    let key = config.key.as_bytes().to_vec();
    Ok(task::spawn_blocking(move || decode_sync(token, key)).await??)
}
fn decode_sync(token: String, key: Vec<u8>) -> Result<Option<String>> {
    match jsonwebtoken::decode::<Claims>(
        &token,
        &DecodingKey::from_secret(&key),
        &Default::default(),
    ) {
        Ok(data) => Ok(Some(data.claims.sub)),
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
