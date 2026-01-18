use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::rust::double_option;
use sqlx::{Executor, FromRow, Sqlite, SqlitePool};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("Repository not found")]
    NotFound,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct Repo {
    pub id: Uuid,
    pub path: PathBuf,
    pub name: String,
    pub display_name: String,
    pub setup_script: Option<String>,
    pub cleanup_script: Option<String>,
    pub copy_files: Option<String>,
    pub parallel_setup_script: bool,
    pub dev_server_script: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

/// Represents a repository with effective configuration values.
///
/// This combines database-stored values with values from a vibekanban.json
/// config file (if present). When a config file exists and defines a value,
/// that value takes precedence over the database value.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RepoWithEffectiveConfig {
    /// The base repository ID.
    pub id: Uuid,
    /// The path to the repository on disk.
    pub path: PathBuf,
    /// The repository folder name.
    pub name: String,
    /// The display name shown in the UI.
    pub display_name: String,
    /// Whether a vibekanban.json config file exists in the repository.
    pub has_config_file: bool,
    /// Effective setup script (from config file if present, otherwise from database).
    pub setup_script: Option<String>,
    /// Whether the setup_script value comes from the config file.
    pub setup_script_from_file: bool,
    /// Effective cleanup script.
    pub cleanup_script: Option<String>,
    /// Whether the cleanup_script value comes from the config file.
    pub cleanup_script_from_file: bool,
    /// Effective dev server script.
    pub dev_server_script: Option<String>,
    /// Whether the dev_server_script value comes from the config file.
    pub dev_server_script_from_file: bool,
    /// Effective copy files configuration.
    pub copy_files: Option<String>,
    /// Whether the copy_files value comes from the config file.
    pub copy_files_from_file: bool,
    /// Effective parallel setup script flag.
    pub parallel_setup_script: bool,
    /// Whether the parallel_setup_script value comes from the config file.
    pub parallel_setup_script_from_file: bool,
    /// When the repository was created.
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    /// When the repository was last updated.
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

impl RepoWithEffectiveConfig {
    /// Creates a RepoWithEffectiveConfig from a Repo without any config file overrides.
    pub fn from_repo_without_config(repo: Repo) -> Self {
        Self {
            id: repo.id,
            path: repo.path,
            name: repo.name,
            display_name: repo.display_name,
            has_config_file: false,
            setup_script: repo.setup_script,
            setup_script_from_file: false,
            cleanup_script: repo.cleanup_script,
            cleanup_script_from_file: false,
            dev_server_script: repo.dev_server_script,
            dev_server_script_from_file: false,
            copy_files: repo.copy_files,
            copy_files_from_file: false,
            parallel_setup_script: repo.parallel_setup_script,
            parallel_setup_script_from_file: false,
            created_at: repo.created_at,
            updated_at: repo.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct UpdateRepo {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    #[ts(optional, type = "string | null")]
    pub display_name: Option<Option<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    #[ts(optional, type = "string | null")]
    pub setup_script: Option<Option<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    #[ts(optional, type = "string | null")]
    pub cleanup_script: Option<Option<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    #[ts(optional, type = "string | null")]
    pub copy_files: Option<Option<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    #[ts(optional, type = "boolean | null")]
    pub parallel_setup_script: Option<Option<bool>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    #[ts(optional, type = "string | null")]
    pub dev_server_script: Option<Option<String>>,
}

impl Repo {
    /// Get repos that still have the migration sentinel as their name.
    /// Used by the startup backfill to fix repo names.
    pub async fn list_needing_name_fix(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Repo,
            r#"SELECT id as "id!: Uuid",
                      path,
                      name,
                      display_name,
                      setup_script,
                      cleanup_script,
                      copy_files,
                      parallel_setup_script as "parallel_setup_script!: bool",
                      dev_server_script,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM repos
               WHERE name = '__NEEDS_BACKFILL__'"#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn update_name(
        pool: &SqlitePool,
        id: Uuid,
        name: &str,
        display_name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE repos SET name = $1, display_name = $2, updated_at = datetime('now', 'subsec') WHERE id = $3",
            name,
            display_name,
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Repo,
            r#"SELECT id as "id!: Uuid",
                      path,
                      name,
                      display_name,
                      setup_script,
                      cleanup_script,
                      copy_files,
                      parallel_setup_script as "parallel_setup_script!: bool",
                      dev_server_script,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM repos
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_ids(pool: &SqlitePool, ids: &[Uuid]) -> Result<Vec<Self>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch each repo individually since SQLite doesn't support array parameters
        let mut repos = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(repo) = Self::find_by_id(pool, *id).await? {
                repos.push(repo);
            }
        }
        Ok(repos)
    }

    pub async fn find_or_create<'e, E>(
        executor: E,
        path: &Path,
        display_name: &str,
    ) -> Result<Self, sqlx::Error>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let path_str = path.to_string_lossy().to_string();
        let id = Uuid::new_v4();
        let repo_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| id.to_string());

        // Use INSERT OR IGNORE + SELECT to handle race conditions atomically
        sqlx::query_as!(
            Repo,
            r#"INSERT INTO repos (id, path, name, display_name)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT(path) DO UPDATE SET updated_at = updated_at
               RETURNING id as "id!: Uuid",
                         path,
                         name,
                         display_name,
                         setup_script,
                         cleanup_script,
                         copy_files,
                         parallel_setup_script as "parallel_setup_script!: bool",
                         dev_server_script,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            path_str,
            repo_name,
            display_name,
        )
        .fetch_one(executor)
        .await
    }

    pub async fn delete_orphaned(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"DELETE FROM repos
               WHERE id NOT IN (SELECT repo_id FROM project_repos)
                 AND id NOT IN (SELECT repo_id FROM workspace_repos)"#
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Repo,
            r#"SELECT id as "id!: Uuid",
                      path,
                      name,
                      display_name,
                      setup_script,
                      cleanup_script,
                      copy_files,
                      parallel_setup_script as "parallel_setup_script!: bool",
                      dev_server_script,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM repos
               ORDER BY display_name ASC"#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        payload: &UpdateRepo,
    ) -> Result<Self, RepoError> {
        let existing = Self::find_by_id(pool, id)
            .await?
            .ok_or(RepoError::NotFound)?;

        // None = don't update (use existing)
        // Some(None) = set to NULL
        // Some(Some(v)) = set to v
        let display_name = match &payload.display_name {
            None => existing.display_name,
            Some(v) => v.clone().unwrap_or_default(),
        };
        let setup_script = match &payload.setup_script {
            None => existing.setup_script,
            Some(v) => v.clone(),
        };
        let cleanup_script = match &payload.cleanup_script {
            None => existing.cleanup_script,
            Some(v) => v.clone(),
        };
        let copy_files = match &payload.copy_files {
            None => existing.copy_files,
            Some(v) => v.clone(),
        };
        let parallel_setup_script = match &payload.parallel_setup_script {
            None => existing.parallel_setup_script,
            Some(v) => v.unwrap_or(false),
        };
        let dev_server_script = match &payload.dev_server_script {
            None => existing.dev_server_script,
            Some(v) => v.clone(),
        };

        sqlx::query_as!(
            Repo,
            r#"UPDATE repos
               SET display_name = $1,
                   setup_script = $2,
                   cleanup_script = $3,
                   copy_files = $4,
                   parallel_setup_script = $5,
                   dev_server_script = $6,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $7
               RETURNING id as "id!: Uuid",
                         path,
                         name,
                         display_name,
                         setup_script,
                         cleanup_script,
                         copy_files,
                         parallel_setup_script as "parallel_setup_script!: bool",
                         dev_server_script,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            display_name,
            setup_script,
            cleanup_script,
            copy_files,
            parallel_setup_script,
            dev_server_script,
            id
        )
        .fetch_one(pool)
        .await
        .map_err(RepoError::from)
    }
}
