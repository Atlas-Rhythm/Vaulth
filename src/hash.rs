use crate::config::HashConfig;
use anyhow::{anyhow, Result};
use argon2::{Config, ThreadMode, Variant, Version};
use rand::{rngs::OsRng, Rng};
use tokio::task;

/// Hashes a password
pub async fn hash(password: &str, config: HashConfig) -> Result<String> {
    log::debug!("hashing password");

    let password = password.as_bytes().to_vec();

    let mut argon_config = Config {
        variant: Variant::Argon2i,
        version: Version::Version13,
        ..Default::default()
    };
    if let Some(hash_len) = config.hash_len {
        argon_config.hash_length = hash_len;
    }
    if let Some(lanes) = config.lanes {
        argon_config.lanes = lanes;
        if lanes > 1 {
            argon_config.thread_mode = ThreadMode::Parallel;
        }
    }
    if let Some(mem_cost) = config.mem_cost {
        argon_config.mem_cost = mem_cost;
    }
    if let Some(time_cost) = config.time_cost {
        argon_config.time_cost = time_cost;
    }

    Ok(task::spawn_blocking(move || {
        hash_sync(
            password,
            argon_config,
            config.salt_len.unwrap_or(16),
            config.secret.as_ref().map(String::as_bytes).unwrap_or(&[]),
        )
    })
    .await??)
}
fn hash_sync<'a>(
    password: Vec<u8>,
    mut config: Config<'a>,
    salt_len: usize,
    secret: &'a [u8],
) -> Result<String> {
    let mut salt = vec![0; salt_len];
    OsRng.fill(salt.as_mut_slice());

    config.secret = secret;

    let hash = argon2::hash_encoded(&password, &salt, &config)?;
    if hash.len() > 255 {
        return Err(anyhow!(
            "hash length exceeds maximum length supported in the database"
        ));
    }

    Ok(hash)
}

/// Verifies a password hash
pub async fn verify(hash: String, password: &str, secret: &str) -> Result<bool> {
    log::debug!("verifying password hash");

    let password = password.as_bytes().to_vec();
    let secret = secret.as_bytes().to_vec();
    Ok(task::spawn_blocking(move || verify_sync(hash, password, secret)).await??)
}
fn verify_sync(hash: String, password: Vec<u8>, secret: Vec<u8>) -> Result<bool> {
    Ok(argon2::verify_encoded_ext(&hash, &password, &secret, &[])?)
}
