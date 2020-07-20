use crate::DbConnection;
use chrono::NaiveDateTime;
use sqlx::{Pool, Row};

pub struct User {
    pub id: String,

    pub inserted_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub login_at: NaiveDateTime,

    pub display_name: Option<String>,
    pub about: Option<String>,

    pub password: Option<String>,

    pub google_id: Option<String>,
    pub microsoft_id: Option<String>,
    pub facebook_id: Option<String>,
    pub twitter_id: Option<String>,
    pub github_id: Option<String>,
    pub discord_id: Option<String>,
}

#[cfg(feature = "postgres")]
mod postgres {
    use super::*;
    use sqlx::postgres::PgRow;

    impl User {
        #[tracing::instrument(skip(pool))]
        pub async fn select(id: &str, pool: &Pool<DbConnection>) -> sqlx::Result<Option<Self>> {
            tracing::debug!("selecting user by id");

            sqlx::query_as!(Self, "SELECT * FROM vaulth WHERE id = $1", id)
                .fetch_optional(pool)
                .await
        }

        #[tracing::instrument(skip(pool))]
        pub async fn delete(id: &str, pool: &Pool<DbConnection>) -> sqlx::Result<Option<Self>> {
            tracing::debug!("deleting user by id");

            sqlx::query_as!(Self, "DELETE FROM vaulth WHERE id = $1 RETURNING *", id)
                .fetch_optional(pool)
                .await
        }

        #[tracing::instrument(skip(pool))]
        pub async fn select_by_provider(
            name: &str,
            id: &str,
            pool: &Pool<DbConnection>,
        ) -> sqlx::Result<Option<String>> {
            tracing::debug!("selecting user by provider");

            sqlx::query(&format!("SELECT id FROM vaulth WHERE {}_id = $1", name))
                .bind(id)
                .map(|r: PgRow| r.get("id"))
                .fetch_optional(pool)
                .await
        }
    }
}

#[cfg(feature = "mysql")]
mod mysql {
    use super::*;
    use sqlx::mysql::MySqlRow;

    impl User {
        #[tracing::instrument(skip(pool))]
        pub async fn select(id: &str, pool: &Pool<DbConnection>) -> sqlx::Result<Option<Self>> {
            tracing::debug!("selecting user by id");

            sqlx::query_as!(Self, "SELECT * FROM vaulth WHERE id = ?", id)
                .fetch_optional(pool)
                .await
        }

        #[tracing::instrument(skip(pool))]
        pub async fn delete(id: &str, pool: &Pool<DbConnection>) -> sqlx::Result<Option<Self>> {
            tracing::debug!("deleting user by id");

            sqlx::query_as!(Self, "DELETE FROM vaulth WHERE id = ? RETURNING *", id)
                .fetch_optional(pool)
                .await
        }

        #[tracing::instrument(skip(pool))]
        pub async fn select_by_provider(
            name: &str,
            id: &str,
            pool: &Pool<DbConnection>,
        ) -> sqlx::Result<Option<String>> {
            tracing::debug!("selecting user by provider");

            sqlx::query(&format!("SELECT id FROM vaulth WHERE {}_id = ?", name))
                .bind(id)
                .map(|r: MySqlRow| r.get("id"))
                .fetch_optional(pool)
                .await
        }
    }
}
