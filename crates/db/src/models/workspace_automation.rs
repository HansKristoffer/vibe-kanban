use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq, TS)]
#[sqlx(type_name = "workspace_automation_mode", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum WorkspaceAutomationMode {
    Ralph,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq, TS)]
#[sqlx(type_name = "workspace_automation_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum WorkspaceAutomationStatus {
    Running,
    Paused,
    Stopped,
    Completed,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorkspaceAutomation {
    pub workspace_id: Uuid,
    pub mode: WorkspaceAutomationMode,
    pub status: WorkspaceAutomationStatus,
    pub iteration: i64,
    pub max_iterations: i64,
    pub consecutive_failures: i64,
    pub max_failures: i64,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateWorkspaceAutomation {
    pub mode: WorkspaceAutomationMode,
    pub status: WorkspaceAutomationStatus,
    pub iteration: i64,
    pub max_iterations: i64,
    pub consecutive_failures: i64,
    pub max_failures: i64,
    pub last_error: Option<String>,
}

impl WorkspaceAutomation {
    pub async fn find_by_workspace_id(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceAutomation,
            r#"SELECT
                workspace_id as "workspace_id!: Uuid",
                mode as "mode!: WorkspaceAutomationMode",
                status as "status!: WorkspaceAutomationStatus",
                iteration as "iteration!: i64",
                max_iterations as "max_iterations!: i64",
                consecutive_failures as "consecutive_failures!: i64",
                max_failures as "max_failures!: i64",
                last_error,
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>"
              FROM workspace_automations
              WHERE workspace_id = $1"#,
            workspace_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        workspace_id: Uuid,
        data: &CreateWorkspaceAutomation,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceAutomation,
            r#"INSERT INTO workspace_automations (
                workspace_id,
                mode,
                status,
                iteration,
                max_iterations,
                consecutive_failures,
                max_failures,
                last_error
              )
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
              RETURNING
                workspace_id as "workspace_id!: Uuid",
                mode as "mode!: WorkspaceAutomationMode",
                status as "status!: WorkspaceAutomationStatus",
                iteration as "iteration!: i64",
                max_iterations as "max_iterations!: i64",
                consecutive_failures as "consecutive_failures!: i64",
                max_failures as "max_failures!: i64",
                last_error,
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>""#,
            workspace_id,
            data.mode,
            data.status,
            data.iteration,
            data.max_iterations,
            data.consecutive_failures,
            data.max_failures,
            data.last_error
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update_status(
        pool: &SqlitePool,
        workspace_id: Uuid,
        status: WorkspaceAutomationStatus,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE workspace_automations
               SET status = $2, updated_at = datetime('now', 'subsec')
               WHERE workspace_id = $1"#,
            workspace_id,
            status
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn start_or_reset(
        pool: &SqlitePool,
        workspace_id: Uuid,
        max_iterations: i64,
        max_failures: i64,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceAutomation,
            r#"INSERT INTO workspace_automations (
                workspace_id,
                mode,
                status,
                iteration,
                max_iterations,
                consecutive_failures,
                max_failures,
                last_error
              )
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
              ON CONFLICT(workspace_id) DO UPDATE SET
                status = excluded.status,
                iteration = excluded.iteration,
                max_iterations = excluded.max_iterations,
                consecutive_failures = excluded.consecutive_failures,
                max_failures = excluded.max_failures,
                last_error = excluded.last_error,
                updated_at = datetime('now', 'subsec')
              RETURNING
                workspace_id as "workspace_id!: Uuid",
                mode as "mode!: WorkspaceAutomationMode",
                status as "status!: WorkspaceAutomationStatus",
                iteration as "iteration!: i64",
                max_iterations as "max_iterations!: i64",
                consecutive_failures as "consecutive_failures!: i64",
                max_failures as "max_failures!: i64",
                last_error,
                created_at as "created_at!: DateTime<Utc>",
                updated_at as "updated_at!: DateTime<Utc>""#,
            workspace_id,
            WorkspaceAutomationMode::Ralph,
            WorkspaceAutomationStatus::Running,
            1,
            max_iterations,
            0,
            max_failures,
            None::<String>
        )
        .fetch_one(pool)
        .await
    }

    pub async fn increment_iteration(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE workspace_automations
               SET iteration = iteration + 1,
                   updated_at = datetime('now', 'subsec')
               WHERE workspace_id = $1"#,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn reset_failures(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE workspace_automations
               SET consecutive_failures = 0,
                   last_error = NULL,
                   updated_at = datetime('now', 'subsec')
               WHERE workspace_id = $1"#,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn record_failure(
        pool: &SqlitePool,
        workspace_id: Uuid,
        last_error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE workspace_automations
               SET consecutive_failures = consecutive_failures + 1,
                   last_error = $2,
                   updated_at = datetime('now', 'subsec')
               WHERE workspace_id = $1"#,
            workspace_id,
            last_error
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
