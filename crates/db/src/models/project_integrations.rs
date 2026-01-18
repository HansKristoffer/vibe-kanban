use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct ProjectIntegrations {
    pub project_id: Uuid,
    pub webhook_token: String,
    pub linear_api_key: Option<String>,
    pub linear_team_id: Option<String>,
    pub linear_state_id_todo: Option<String>,
    pub linear_state_id_inprogress: Option<String>,
    pub linear_state_id_inreview: Option<String>,
    pub linear_state_id_done: Option<String>,
    pub linear_state_id_cancelled: Option<String>,
    pub linear_webhook_secret: Option<String>,
    pub intercom_access_token: Option<String>,
    pub intercom_webhook_secret: Option<String>,
    pub intercom_admin_id: Option<String>,
    pub modjo_api_key: Option<String>,
    pub modjo_webhook_secret: Option<String>,
    pub posthog_webhook_secret: Option<String>,
    pub sentry_webhook_secret: Option<String>,
    pub posthog_api_key: Option<String>,
    pub posthog_host: Option<String>,
    pub posthog_project_id: Option<String>,
    pub sentry_api_token: Option<String>,
    pub sentry_org_slug: Option<String>,
    pub sentry_project_slug: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpsertProjectIntegrations {
    pub webhook_token: String,
    pub linear_api_key: Option<String>,
    pub linear_team_id: Option<String>,
    pub linear_state_id_todo: Option<String>,
    pub linear_state_id_inprogress: Option<String>,
    pub linear_state_id_inreview: Option<String>,
    pub linear_state_id_done: Option<String>,
    pub linear_state_id_cancelled: Option<String>,
    pub linear_webhook_secret: Option<String>,
    pub intercom_access_token: Option<String>,
    pub intercom_webhook_secret: Option<String>,
    pub intercom_admin_id: Option<String>,
    pub modjo_api_key: Option<String>,
    pub modjo_webhook_secret: Option<String>,
    pub posthog_webhook_secret: Option<String>,
    pub sentry_webhook_secret: Option<String>,
    pub posthog_api_key: Option<String>,
    pub posthog_host: Option<String>,
    pub posthog_project_id: Option<String>,
    pub sentry_api_token: Option<String>,
    pub sentry_org_slug: Option<String>,
    pub sentry_project_slug: Option<String>,
}

impl ProjectIntegrations {
    pub async fn find_by_project_id(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectIntegrations,
            r#"SELECT project_id as "project_id!: Uuid",
                      webhook_token,
                      linear_api_key,
                      linear_team_id,
                      linear_state_id_todo,
                      linear_state_id_inprogress,
                      linear_state_id_inreview,
                      linear_state_id_done,
                      linear_state_id_cancelled,
                      linear_webhook_secret,
                      intercom_access_token,
                      intercom_webhook_secret,
                      intercom_admin_id,
                      modjo_api_key,
                      modjo_webhook_secret,
                      posthog_webhook_secret,
                      sentry_webhook_secret,
                      posthog_api_key,
                      posthog_host,
                      posthog_project_id,
                      sentry_api_token,
                      sentry_org_slug,
                      sentry_project_slug,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM project_integrations
               WHERE project_id = $1"#,
            project_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectIntegrations,
            r#"SELECT project_id as "project_id!: Uuid",
                      webhook_token,
                      linear_api_key,
                      linear_team_id,
                      linear_state_id_todo,
                      linear_state_id_inprogress,
                      linear_state_id_inreview,
                      linear_state_id_done,
                      linear_state_id_cancelled,
                      linear_webhook_secret,
                      intercom_access_token,
                      intercom_webhook_secret,
                      intercom_admin_id,
                      modjo_api_key,
                      modjo_webhook_secret,
                      posthog_webhook_secret,
                      sentry_webhook_secret,
                      posthog_api_key,
                      posthog_host,
                      posthog_project_id,
                      sentry_api_token,
                      sentry_org_slug,
                      sentry_project_slug,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM project_integrations"#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn upsert(
        pool: &SqlitePool,
        project_id: Uuid,
        data: &UpsertProjectIntegrations,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            ProjectIntegrations,
            r#"INSERT INTO project_integrations (
                    project_id,
                    webhook_token,
                    linear_api_key,
                    linear_team_id,
                    linear_state_id_todo,
                    linear_state_id_inprogress,
                    linear_state_id_inreview,
                    linear_state_id_done,
                    linear_state_id_cancelled,
                    linear_webhook_secret,
                    intercom_access_token,
                    intercom_webhook_secret,
                    intercom_admin_id,
                    modjo_api_key,
                    modjo_webhook_secret,
                    posthog_webhook_secret,
                    sentry_webhook_secret,
                    posthog_api_key,
                    posthog_host,
                    posthog_project_id,
                    sentry_api_token,
                    sentry_org_slug,
                    sentry_project_slug
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                    $18, $19, $20, $21, $22, $23
                )
                ON CONFLICT(project_id) DO UPDATE SET
                    webhook_token = excluded.webhook_token,
                    linear_api_key = excluded.linear_api_key,
                    linear_team_id = excluded.linear_team_id,
                    linear_state_id_todo = excluded.linear_state_id_todo,
                    linear_state_id_inprogress = excluded.linear_state_id_inprogress,
                    linear_state_id_inreview = excluded.linear_state_id_inreview,
                    linear_state_id_done = excluded.linear_state_id_done,
                    linear_state_id_cancelled = excluded.linear_state_id_cancelled,
                    linear_webhook_secret = excluded.linear_webhook_secret,
                    intercom_access_token = excluded.intercom_access_token,
                    intercom_webhook_secret = excluded.intercom_webhook_secret,
                    intercom_admin_id = excluded.intercom_admin_id,
                    modjo_api_key = excluded.modjo_api_key,
                    modjo_webhook_secret = excluded.modjo_webhook_secret,
                    posthog_webhook_secret = excluded.posthog_webhook_secret,
                    sentry_webhook_secret = excluded.sentry_webhook_secret,
                    posthog_api_key = excluded.posthog_api_key,
                    posthog_host = excluded.posthog_host,
                    posthog_project_id = excluded.posthog_project_id,
                    sentry_api_token = excluded.sentry_api_token,
                    sentry_org_slug = excluded.sentry_org_slug,
                    sentry_project_slug = excluded.sentry_project_slug,
                    updated_at = datetime('now', 'subsec')
                RETURNING project_id as "project_id!: Uuid",
                          webhook_token,
                          linear_api_key,
                          linear_team_id,
                          linear_state_id_todo,
                          linear_state_id_inprogress,
                          linear_state_id_inreview,
                          linear_state_id_done,
                          linear_state_id_cancelled,
                          linear_webhook_secret,
                          intercom_access_token,
                          intercom_webhook_secret,
                          intercom_admin_id,
                          modjo_api_key,
                          modjo_webhook_secret,
                          posthog_webhook_secret,
                          sentry_webhook_secret,
                          posthog_api_key,
                          posthog_host,
                          posthog_project_id,
                          sentry_api_token,
                          sentry_org_slug,
                          sentry_project_slug,
                          created_at as "created_at!: DateTime<Utc>",
                          updated_at as "updated_at!: DateTime<Utc>""#,
            project_id,
            data.webhook_token,
            data.linear_api_key,
            data.linear_team_id,
            data.linear_state_id_todo,
            data.linear_state_id_inprogress,
            data.linear_state_id_inreview,
            data.linear_state_id_done,
            data.linear_state_id_cancelled,
            data.linear_webhook_secret,
            data.intercom_access_token,
            data.intercom_webhook_secret,
            data.intercom_admin_id,
            data.modjo_api_key,
            data.modjo_webhook_secret,
            data.posthog_webhook_secret,
            data.sentry_webhook_secret,
            data.posthog_api_key,
            data.posthog_host,
            data.posthog_project_id,
            data.sentry_api_token,
            data.sentry_org_slug,
            data.sentry_project_slug
        )
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_webhook_token(
        pool: &SqlitePool,
        webhook_token: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectIntegrations,
            r#"SELECT project_id as "project_id!: Uuid",
                      webhook_token,
                      linear_api_key,
                      linear_team_id,
                      linear_state_id_todo,
                      linear_state_id_inprogress,
                      linear_state_id_inreview,
                      linear_state_id_done,
                      linear_state_id_cancelled,
                      linear_webhook_secret,
                      intercom_access_token,
                      intercom_webhook_secret,
                      intercom_admin_id,
                      modjo_api_key,
                      modjo_webhook_secret,
                      posthog_webhook_secret,
                      sentry_webhook_secret,
                      posthog_api_key,
                      posthog_host,
                      posthog_project_id,
                      sentry_api_token,
                      sentry_org_slug,
                      sentry_project_slug,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM project_integrations
               WHERE webhook_token = $1"#,
            webhook_token
        )
        .fetch_optional(pool)
        .await
    }
}
