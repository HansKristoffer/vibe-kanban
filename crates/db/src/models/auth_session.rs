use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AuthSession {
    pub id: String,
    pub user_id: Uuid,
    #[ts(type = "Date")]
    pub expires_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
}

impl AuthSession {
    pub async fn find_by_id(
        pool: &SqlitePool,
        session_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            AuthSession,
            r#"SELECT id as "id!: String",
                      user_id as "user_id!: Uuid",
                      expires_at as "expires_at!: DateTime<Utc>",
                      created_at as "created_at!: DateTime<Utc>"
               FROM auth_sessions
               WHERE id = $1"#,
            session_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        session_id: &str,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            AuthSession,
            r#"INSERT INTO auth_sessions (id, user_id, expires_at)
               VALUES ($1, $2, $3)
               RETURNING id as "id!: String",
                         user_id as "user_id!: Uuid",
                         expires_at as "expires_at!: DateTime<Utc>",
                         created_at as "created_at!: DateTime<Utc>""#,
            session_id,
            user_id,
            expires_at
        )
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, session_id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM auth_sessions WHERE id = $1", session_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_expired(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM auth_sessions WHERE expires_at <= datetime('now', 'subsec')"
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}
