use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Type, Serialize, Deserialize, TS)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InboxSourceCursorType {
    Linear,
    Intercom,
    Modjo,
    Manual,
    Posthog,
    Sentry,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct InboxSourceCursor {
    pub project_id: Uuid,
    pub source: InboxSourceCursorType,
    pub cursor: Option<String>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

impl InboxSourceCursor {
    pub async fn find(
        pool: &SqlitePool,
        project_id: Uuid,
        source: InboxSourceCursorType,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            InboxSourceCursor,
            r#"SELECT project_id as "project_id!: Uuid",
                      source as "source!: InboxSourceCursorType",
                      cursor,
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM inbox_source_cursors
               WHERE project_id = $1 AND source = $2"#,
            project_id,
            source
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn upsert(
        pool: &SqlitePool,
        project_id: Uuid,
        source: InboxSourceCursorType,
        cursor: Option<String>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            InboxSourceCursor,
            r#"INSERT INTO inbox_source_cursors (
                    project_id,
                    source,
                    cursor
                ) VALUES ($1, $2, $3)
                ON CONFLICT(project_id, source) DO UPDATE SET
                    cursor = excluded.cursor,
                    updated_at = datetime('now', 'subsec')
                RETURNING project_id as "project_id!: Uuid",
                          source as "source!: InboxSourceCursorType",
                          cursor,
                          updated_at as "updated_at!: DateTime<Utc>""#,
            project_id,
            source,
            cursor
        )
        .fetch_one(pool)
        .await
    }
}
