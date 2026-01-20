use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertAuthUser {
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
}

impl AuthUser {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            AuthUser,
            r#"SELECT id as "id!: Uuid",
                      email,
                      name,
                      picture_url,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM auth_users
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_email(
        pool: &SqlitePool,
        email: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            AuthUser,
            r#"SELECT id as "id!: Uuid",
                      email,
                      name,
                      picture_url,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM auth_users
               WHERE email = $1"#,
            email
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn upsert(
        pool: &SqlitePool,
        data: &UpsertAuthUser,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as!(
            AuthUser,
            r#"INSERT INTO auth_users (id, email, name, picture_url)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT(email) DO UPDATE SET
                   name = excluded.name,
                   picture_url = excluded.picture_url,
                   updated_at = datetime('now', 'subsec')
               RETURNING id as "id!: Uuid",
                         email,
                         name,
                         picture_url,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            data.email,
            data.name,
            data.picture_url
        )
        .fetch_one(pool)
        .await
    }
}
