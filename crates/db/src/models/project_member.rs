use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow, Sqlite, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectMember {
    pub project_id: Uuid,
    pub email: String,
    pub role: String,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
}

impl ProjectMember {
    pub async fn list_by_project_id(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectMember,
            r#"SELECT project_id as "project_id!: Uuid",
                      email,
                      role,
                      created_at as "created_at!: DateTime<Utc>"
               FROM project_members
               WHERE project_id = $1
               ORDER BY email ASC"#,
            project_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn add_member(
        pool: &SqlitePool,
        project_id: Uuid,
        email: &str,
        role: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            ProjectMember,
            r#"INSERT INTO project_members (project_id, email, role)
               VALUES ($1, $2, $3)
               ON CONFLICT(project_id, email) DO UPDATE SET
                   role = excluded.role
               RETURNING project_id as "project_id!: Uuid",
                         email,
                         role,
                         created_at as "created_at!: DateTime<Utc>""#,
            project_id,
            email,
            role
        )
        .fetch_one(pool)
        .await
    }

    pub async fn add_member_tx<'e, E>(
        executor: E,
        project_id: Uuid,
        email: &str,
        role: &str,
    ) -> Result<Self, sqlx::Error>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as!(
            ProjectMember,
            r#"INSERT INTO project_members (project_id, email, role)
               VALUES ($1, $2, $3)
               ON CONFLICT(project_id, email) DO UPDATE SET
                   role = excluded.role
               RETURNING project_id as "project_id!: Uuid",
                         email,
                         role,
                         created_at as "created_at!: DateTime<Utc>""#,
            project_id,
            email,
            role
        )
        .fetch_one(executor)
        .await
    }

    pub async fn remove_member(
        pool: &SqlitePool,
        project_id: Uuid,
        email: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM project_members WHERE project_id = $1 AND email = $2",
            project_id,
            email
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn is_member(
        pool: &SqlitePool,
        project_id: Uuid,
        email: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                   SELECT 1 FROM project_members WHERE project_id = $1 AND email = $2
               ) as "exists!: i64""#,
            project_id,
            email
        )
        .fetch_one(pool)
        .await?;
        Ok(result == 1)
    }

    pub async fn list_project_ids_for_email(
        pool: &SqlitePool,
        email: &str,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT project_id as "project_id!: Uuid"
               FROM project_members
               WHERE email = $1"#,
            email
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|row| row.project_id).collect())
    }

    /// Find all projects that have no members (orphan projects).
    pub async fn find_orphan_project_ids(pool: &SqlitePool) -> Result<Vec<Uuid>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT p.id as "id!: Uuid"
               FROM projects p
               WHERE NOT EXISTS (
                   SELECT 1 FROM project_members pm WHERE pm.project_id = p.id
               )"#
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|row| row.id).collect())
    }

    /// Assign all orphan projects to the given email as owner.
    /// Returns the number of projects assigned.
    pub async fn assign_orphan_projects_to_user(
        pool: &SqlitePool,
        email: &str,
    ) -> Result<u64, sqlx::Error> {
        let orphan_ids = Self::find_orphan_project_ids(pool).await?;
        let count = orphan_ids.len() as u64;
        for project_id in orphan_ids {
            Self::add_member(pool, project_id, email, "owner").await?;
        }
        Ok(count)
    }
}
