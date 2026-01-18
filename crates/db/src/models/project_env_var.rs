use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

/// A stored environment variable value for a project.
/// The variable name must be allowed by the project's vibekanban.json config.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectEnvVar {
    pub project_id: Uuid,
    pub name: String,
    pub value: String,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

impl ProjectEnvVar {
    /// List all environment variables for a project.
    pub async fn list_by_project_id(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectEnvVar,
            r#"SELECT project_id as "project_id!: Uuid",
                      name,
                      value,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM project_env_vars
               WHERE project_id = $1
               ORDER BY name ASC"#,
            project_id
        )
        .fetch_all(pool)
        .await
    }

    /// Get a single environment variable by project and name.
    pub async fn find_by_project_and_name(
        pool: &SqlitePool,
        project_id: Uuid,
        name: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectEnvVar,
            r#"SELECT project_id as "project_id!: Uuid",
                      name,
                      value,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM project_env_vars
               WHERE project_id = $1 AND name = $2"#,
            project_id,
            name
        )
        .fetch_optional(pool)
        .await
    }

    /// Upsert (insert or update) an environment variable.
    pub async fn upsert(
        pool: &SqlitePool,
        project_id: Uuid,
        name: &str,
        value: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            ProjectEnvVar,
            r#"INSERT INTO project_env_vars (project_id, name, value)
               VALUES ($1, $2, $3)
               ON CONFLICT(project_id, name) DO UPDATE SET
                   value = excluded.value,
                   updated_at = datetime('now', 'subsec')
               RETURNING project_id as "project_id!: Uuid",
                         name,
                         value,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            project_id,
            name,
            value
        )
        .fetch_one(pool)
        .await
    }

    /// Delete an environment variable.
    pub async fn delete(
        pool: &SqlitePool,
        project_id: Uuid,
        name: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"DELETE FROM project_env_vars WHERE project_id = $1 AND name = $2"#,
            project_id,
            name
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete all environment variables for a project.
    pub async fn delete_all_for_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"DELETE FROM project_env_vars WHERE project_id = $1"#,
            project_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Get values as a HashMap for injection into environment.
    /// Only returns values for names in the allowlist.
    pub async fn get_values_for_names(
        pool: &SqlitePool,
        project_id: Uuid,
        allowed_names: &[String],
    ) -> Result<std::collections::HashMap<String, String>, sqlx::Error> {
        let all_vars = Self::list_by_project_id(pool, project_id).await?;
        let allowed_set: std::collections::HashSet<&String> = allowed_names.iter().collect();

        Ok(all_vars
            .into_iter()
            .filter(|v| allowed_set.contains(&v.name))
            .map(|v| (v.name, v.value))
            .collect())
    }
}
