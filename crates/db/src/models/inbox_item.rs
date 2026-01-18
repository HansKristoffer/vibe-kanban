use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow, Sqlite, SqlitePool, Type};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Type, Serialize, Deserialize, TS)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InboxSource {
    Linear,
    Intercom,
    Modjo,
    Manual,
    Posthog,
    Sentry,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, TS)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InboxItemKind {
    Bug,
    Feature,
    Other,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, TS)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InboxItemStatus {
    Pending,
    Accepted,
    Declined,
    Ignored,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct InboxItem {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source: InboxSource,
    pub source_item_id: String,
    pub source_url: Option<String>,
    pub title: String,
    pub raw_payload_json: Option<String>,
    pub kind: InboxItemKind,
    pub status: InboxItemStatus,
    pub prd_markdown: Option<String>,
    pub task_id: Option<Uuid>,
    pub linear_issue_id: Option<String>,
    pub linear_issue_url: Option<String>,
    pub action_token: String,
    #[ts(type = "Date")]
    pub outbound_registered_at: Option<DateTime<Utc>>,
    #[ts(type = "Date")]
    pub outbound_started_at: Option<DateTime<Utc>>,
    #[ts(type = "Date")]
    pub outbound_pr_created_at: Option<DateTime<Utc>>,
    #[ts(type = "Date")]
    pub outbound_pr_merged_at: Option<DateTime<Utc>>,
    pub outbound_last_error: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateInboxItem {
    pub project_id: Uuid,
    pub source: InboxSource,
    pub source_item_id: String,
    pub source_url: Option<String>,
    pub title: String,
    pub raw_payload_json: Option<String>,
    pub kind: InboxItemKind,
    pub status: InboxItemStatus,
    pub prd_markdown: Option<String>,
    pub action_token: String,
    pub linear_issue_id: Option<String>,
    pub linear_issue_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpsertInboxItem {
    pub project_id: Uuid,
    pub source: InboxSource,
    pub source_item_id: String,
    pub source_url: Option<String>,
    pub title: String,
    pub raw_payload_json: Option<String>,
    pub kind: InboxItemKind,
    pub status: InboxItemStatus,
    pub prd_markdown: Option<String>,
    pub action_token: String,
    pub linear_issue_id: Option<String>,
    pub linear_issue_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateInboxItem {
    pub title: Option<String>,
    pub kind: Option<InboxItemKind>,
    pub status: Option<InboxItemStatus>,
    pub prd_markdown: Option<String>,
    pub task_id: Option<Uuid>,
    pub linear_issue_id: Option<String>,
    pub linear_issue_url: Option<String>,
    pub outbound_last_error: Option<String>,
}

impl InboxItem {
    pub async fn find_by_id(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            InboxItem,
            r#"SELECT id as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      source as "source!: InboxSource",
                      source_item_id,
                      source_url,
                      title,
                      raw_payload_json,
                      kind as "kind!: InboxItemKind",
                      status as "status!: InboxItemStatus",
                      prd_markdown,
                      task_id as "task_id: Uuid",
                      linear_issue_id,
                      linear_issue_url,
                      action_token,
                      outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                      outbound_started_at as "outbound_started_at: DateTime<Utc>",
                      outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                      outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                      outbound_last_error,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM inbox_items
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_action_token(
        pool: &SqlitePool,
        action_token: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            InboxItem,
            r#"SELECT id as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      source as "source!: InboxSource",
                      source_item_id,
                      source_url,
                      title,
                      raw_payload_json,
                      kind as "kind!: InboxItemKind",
                      status as "status!: InboxItemStatus",
                      prd_markdown,
                      task_id as "task_id: Uuid",
                      linear_issue_id,
                      linear_issue_url,
                      action_token,
                      outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                      outbound_started_at as "outbound_started_at: DateTime<Utc>",
                      outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                      outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                      outbound_last_error,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM inbox_items
               WHERE action_token = $1"#,
            action_token
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_task_id(
        pool: &SqlitePool,
        task_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            InboxItem,
            r#"SELECT id as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      source as "source!: InboxSource",
                      source_item_id,
                      source_url,
                      title,
                      raw_payload_json,
                      kind as "kind!: InboxItemKind",
                      status as "status!: InboxItemStatus",
                      prd_markdown,
                      task_id as "task_id: Uuid",
                      linear_issue_id,
                      linear_issue_url,
                      action_token,
                      outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                      outbound_started_at as "outbound_started_at: DateTime<Utc>",
                      outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                      outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                      outbound_last_error,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM inbox_items
               WHERE task_id = $1
               LIMIT 1"#,
            task_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn list_by_project_and_status(
        pool: &SqlitePool,
        project_id: Uuid,
        status: InboxItemStatus,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            InboxItem,
            r#"SELECT id as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      source as "source!: InboxSource",
                      source_item_id,
                      source_url,
                      title,
                      raw_payload_json,
                      kind as "kind!: InboxItemKind",
                      status as "status!: InboxItemStatus",
                      prd_markdown,
                      task_id as "task_id: Uuid",
                      linear_issue_id,
                      linear_issue_url,
                      action_token,
                      outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                      outbound_started_at as "outbound_started_at: DateTime<Utc>",
                      outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                      outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                      outbound_last_error,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM inbox_items
               WHERE project_id = $1
                 AND status = $2
               ORDER BY created_at DESC"#,
            project_id,
            status
        )
        .fetch_all(pool)
        .await
    }

    pub async fn list_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            InboxItem,
            r#"SELECT id as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      source as "source!: InboxSource",
                      source_item_id,
                      source_url,
                      title,
                      raw_payload_json,
                      kind as "kind!: InboxItemKind",
                      status as "status!: InboxItemStatus",
                      prd_markdown,
                      task_id as "task_id: Uuid",
                      linear_issue_id,
                      linear_issue_url,
                      action_token,
                      outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                      outbound_started_at as "outbound_started_at: DateTime<Utc>",
                      outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                      outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                      outbound_last_error,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM inbox_items
               WHERE project_id = $1
               ORDER BY created_at DESC"#,
            project_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        executor: impl Executor<'_, Database = Sqlite>,
        data: &CreateInboxItem,
        id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            InboxItem,
            r#"INSERT INTO inbox_items (
                    id,
                    project_id,
                    source,
                    source_item_id,
                    source_url,
                    title,
                    raw_payload_json,
                    kind,
                    status,
                    prd_markdown,
                    linear_issue_id,
                    linear_issue_url,
                    action_token
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                RETURNING id as "id!: Uuid",
                          project_id as "project_id!: Uuid",
                          source as "source!: InboxSource",
                          source_item_id,
                          source_url,
                          title,
                          raw_payload_json,
                          kind as "kind!: InboxItemKind",
                          status as "status!: InboxItemStatus",
                          prd_markdown,
                          task_id as "task_id: Uuid",
                          linear_issue_id,
                          linear_issue_url,
                          action_token,
                          outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                          outbound_started_at as "outbound_started_at: DateTime<Utc>",
                          outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                          outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                          outbound_last_error,
                          created_at as "created_at!: DateTime<Utc>",
                          updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            data.project_id,
            data.source,
            data.source_item_id,
            data.source_url,
            data.title,
            data.raw_payload_json,
            data.kind,
            data.status,
            data.prd_markdown,
            data.linear_issue_id,
            data.linear_issue_url,
            data.action_token
        )
        .fetch_one(executor)
        .await
    }

    pub async fn upsert_by_source(
        pool: &SqlitePool,
        data: &UpsertInboxItem,
        id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            InboxItem,
            r#"INSERT INTO inbox_items (
                    id,
                    project_id,
                    source,
                    source_item_id,
                    source_url,
                    title,
                    raw_payload_json,
                    kind,
                    status,
                    prd_markdown,
                    linear_issue_id,
                    linear_issue_url,
                    action_token
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT(project_id, source, source_item_id) DO UPDATE SET
                    source_url = excluded.source_url,
                    title = excluded.title,
                    raw_payload_json = excluded.raw_payload_json,
                    kind = excluded.kind,
                    status = excluded.status,
                    prd_markdown = excluded.prd_markdown,
                    linear_issue_id = excluded.linear_issue_id,
                    linear_issue_url = excluded.linear_issue_url,
                    updated_at = datetime('now', 'subsec')
                RETURNING id as "id!: Uuid",
                          project_id as "project_id!: Uuid",
                          source as "source!: InboxSource",
                          source_item_id,
                          source_url,
                          title,
                          raw_payload_json,
                          kind as "kind!: InboxItemKind",
                          status as "status!: InboxItemStatus",
                          prd_markdown,
                          task_id as "task_id: Uuid",
                          linear_issue_id,
                          linear_issue_url,
                          action_token,
                          outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                          outbound_started_at as "outbound_started_at: DateTime<Utc>",
                          outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                          outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                          outbound_last_error,
                          created_at as "created_at!: DateTime<Utc>",
                          updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            data.project_id,
            data.source,
            data.source_item_id,
            data.source_url,
            data.title,
            data.raw_payload_json,
            data.kind,
            data.status,
            data.prd_markdown,
            data.linear_issue_id,
            data.linear_issue_url,
            data.action_token
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        payload: &UpdateInboxItem,
    ) -> Result<Self, sqlx::Error> {
        let existing = Self::find_by_id(pool, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;

        let title = payload.title.clone().unwrap_or(existing.title);
        let kind = payload.kind.clone().unwrap_or(existing.kind);
        let status = payload.status.clone().unwrap_or(existing.status);
        let prd_markdown = payload.prd_markdown.clone().or(existing.prd_markdown);
        let task_id = payload.task_id.or(existing.task_id);
        let linear_issue_id = payload.linear_issue_id.clone().or(existing.linear_issue_id);
        let linear_issue_url = payload.linear_issue_url.clone().or(existing.linear_issue_url);
        let outbound_last_error = payload
            .outbound_last_error
            .clone()
            .or(existing.outbound_last_error);

        sqlx::query_as!(
            InboxItem,
            r#"UPDATE inbox_items
               SET title = $2,
                   kind = $3,
                   status = $4,
                   prd_markdown = $5,
                   task_id = $6,
                   linear_issue_id = $7,
                   linear_issue_url = $8,
                   outbound_last_error = $9,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id as "id!: Uuid",
                         project_id as "project_id!: Uuid",
                         source as "source!: InboxSource",
                         source_item_id,
                         source_url,
                         title,
                         raw_payload_json,
                         kind as "kind!: InboxItemKind",
                         status as "status!: InboxItemStatus",
                         prd_markdown,
                         task_id as "task_id: Uuid",
                         linear_issue_id,
                         linear_issue_url,
                         action_token,
                         outbound_registered_at as "outbound_registered_at: DateTime<Utc>",
                         outbound_started_at as "outbound_started_at: DateTime<Utc>",
                         outbound_pr_created_at as "outbound_pr_created_at: DateTime<Utc>",
                         outbound_pr_merged_at as "outbound_pr_merged_at: DateTime<Utc>",
                         outbound_last_error,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            title,
            kind,
            status,
            prd_markdown,
            task_id,
            linear_issue_id,
            linear_issue_url,
            outbound_last_error
        )
        .fetch_one(pool)
        .await
    }

    pub async fn set_outbound_registered(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE inbox_items
               SET outbound_registered_at = datetime('now', 'subsec'),
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn set_outbound_started(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE inbox_items
               SET outbound_started_at = datetime('now', 'subsec'),
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn set_outbound_pr_created(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE inbox_items
               SET outbound_pr_created_at = datetime('now', 'subsec'),
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn set_outbound_pr_merged(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE inbox_items
               SET outbound_pr_merged_at = datetime('now', 'subsec'),
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn set_outbound_error(
        pool: &SqlitePool,
        id: Uuid,
        message: Option<String>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE inbox_items
               SET outbound_last_error = $2,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            id,
            message
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::project::{CreateProject, Project};
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("failed to create sqlite pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("failed to run migrations");
        pool
    }

    #[tokio::test]
    async fn upsert_is_idempotent_for_source_item() {
        let pool = setup_pool().await;
        let project = Project::create(
            &pool,
            &CreateProject {
                name: "Inbox test".to_string(),
            },
            Uuid::new_v4(),
        )
        .await
        .expect("create project");

        let data = UpsertInboxItem {
            project_id: project.id,
            source: InboxSource::Manual,
            source_item_id: "manual-1".to_string(),
            source_url: None,
            title: "Initial".to_string(),
            raw_payload_json: None,
            kind: InboxItemKind::Bug,
            status: InboxItemStatus::Pending,
            prd_markdown: None,
            action_token: "token-1".to_string(),
            linear_issue_id: None,
            linear_issue_url: None,
        };

        let first = InboxItem::upsert_by_source(&pool, &data, Uuid::new_v4())
            .await
            .expect("first upsert");
        let second = InboxItem::upsert_by_source(
            &pool,
            &UpsertInboxItem {
                title: "Updated".to_string(),
                action_token: "token-2".to_string(),
                ..data
            },
            Uuid::new_v4(),
        )
        .await
        .expect("second upsert");

        assert_eq!(first.id, second.id);
        assert_eq!(second.title, "Updated");
    }

    #[tokio::test]
    async fn update_status_accept_decline() {
        let pool = setup_pool().await;
        let project = Project::create(
            &pool,
            &CreateProject {
                name: "Inbox test".to_string(),
            },
            Uuid::new_v4(),
        )
        .await
        .expect("create project");

        let item = InboxItem::create(
            &pool,
            &CreateInboxItem {
                project_id: project.id,
                source: InboxSource::Manual,
                source_item_id: "manual-2".to_string(),
                source_url: None,
                title: "Needs review".to_string(),
                raw_payload_json: None,
                kind: InboxItemKind::Feature,
                status: InboxItemStatus::Pending,
                prd_markdown: None,
                action_token: "token-3".to_string(),
                linear_issue_id: None,
                linear_issue_url: None,
            },
            Uuid::new_v4(),
        )
        .await
        .expect("create inbox item");

        let accepted = InboxItem::update(
            &pool,
            item.id,
            &UpdateInboxItem {
                title: None,
                kind: None,
                status: Some(InboxItemStatus::Accepted),
                prd_markdown: None,
                task_id: None,
                linear_issue_id: None,
                linear_issue_url: None,
                outbound_last_error: None,
            },
        )
        .await
        .expect("accept update");
        assert!(matches!(accepted.status, InboxItemStatus::Accepted));

        let declined = InboxItem::update(
            &pool,
            item.id,
            &UpdateInboxItem {
                title: None,
                kind: None,
                status: Some(InboxItemStatus::Declined),
                prd_markdown: None,
                task_id: None,
                linear_issue_id: None,
                linear_issue_url: None,
                outbound_last_error: None,
            },
        )
        .await
        .expect("decline update");
        assert!(matches!(declined.status, InboxItemStatus::Declined));
    }
}
