use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize, sqlx::FromRow)]
pub struct User {
    pub id: String,

    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,

    #[serde(skip_serializing)]
    pub password: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microsoft_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facebook_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_id: Option<String>,
}

#[inline]
fn now() -> DateTime<Utc> {
    Utc::now()
}

impl User {
    #[tracing::instrument(level = "debug")]
    pub async fn select(id: &str, pool: &PgPool) -> sqlx::Result<Option<Self>> {
        sqlx::query_as!(Self, "SELECT * FROM vaulth WHERE id = $1", id)
            .fetch_optional(pool)
            .await
    }

    #[tracing::instrument(level = "debug")]
    pub async fn delete(id: &str, pool: &PgPool) -> sqlx::Result<Option<Self>> {
        sqlx::query_as!(Self, "DELETE FROM vaulth WHERE id = $1 RETURNING *", id)
            .fetch_optional(pool)
            .await
    }

    #[tracing::instrument(level = "debug")]
    pub async fn select_by_provider(
        name: &str,
        id: &str,
        pool: &PgPool,
    ) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar(&format!("SELECT id FROM vaulth WHERE {}_id = $1", name))
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    #[tracing::instrument(level = "debug")]
    pub async fn register_by_provider(
        id: &str,
        provider_name: &str,
        provider_id: &str,
        pool: &PgPool,
    ) -> sqlx::Result<User> {
        let now = now();

        sqlx::query_as(&format!(
            "
INSERT INTO vaulth (id, inserted_at, updated_at, login_at, {}_id)
VALUES ($1, $2, $3, $4, $5)
RETURNING *
            ",
            provider_name
        ))
        .bind(id)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(provider_id)
        .fetch_one(pool)
        .await
    }
}
