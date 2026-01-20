use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AuthOAuthState {
    pub id: String,
    pub return_to: Option<String>,
    #[ts(type = "Date")]
    pub expires_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
}

impl AuthOAuthState {
    pub async fn create(
        pool: &SqlitePool,
        id: &str,
        return_to: Option<&str>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            AuthOAuthState,
            r#"INSERT INTO auth_oauth_states (id, return_to, expires_at)
               VALUES ($1, $2, $3)
               RETURNING id as "id!: String",
                         return_to,
                         expires_at as "expires_at!: DateTime<Utc>",
                         created_at as "created_at!: DateTime<Utc>""#,
            id,
            return_to,
            expires_at
        )
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            AuthOAuthState,
            r#"SELECT id as "id!: String",
                      return_to,
                      expires_at as "expires_at!: DateTime<Utc>",
                      created_at as "created_at!: DateTime<Utc>"
               FROM auth_oauth_states
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM auth_oauth_states WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_expired(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM auth_oauth_states WHERE expires_at <= datetime('now', 'subsec')"
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}
