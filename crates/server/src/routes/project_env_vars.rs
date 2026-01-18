use axum::{Extension, Json, Router, extract::State, routing::get};
use db::models::project::Project;
use db::models::project_env_var::ProjectEnvVar;
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::repo_config::collect_env_vars_for_repos;
use std::collections::HashMap;
use ts_rs::TS;

use crate::{DeploymentImpl, error::ApiError};
use utils::response::ApiResponse;

/// Single environment variable entry in the response.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct EnvVarEntry {
    /// The variable name.
    pub name: String,
    /// Whether a value has been configured for this variable.
    pub configured: bool,
}

/// Response for GET /api/projects/:id/env-vars.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ProjectEnvVarsResponse {
    /// List of environment variables from the project's vibekanban.json files.
    pub env_vars: Vec<EnvVarEntry>,
}

/// Request body for PUT /api/projects/:id/env-vars.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateProjectEnvVarsRequest {
    /// Values to set (key = env var name, value = the secret value).
    #[serde(default)]
    pub set: Option<HashMap<String, String>>,
    /// Names of variables to clear/delete.
    #[serde(default)]
    pub clear: Option<Vec<String>>,
}

/// Get the list of environment variables for a project.
///
/// Returns the allowlist from vibekanban.json files and whether each has a value configured.
pub async fn get_project_env_vars(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<ApiResponse<ProjectEnvVarsResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    // Get project repositories
    let repos = deployment
        .project()
        .get_repositories(pool, project.id)
        .await?;

    // Collect allowed env var names from vibekanban.json files
    let allowed_names = collect_env_vars_for_repos(&repos);

    // Get configured values from database
    let configured_vars = ProjectEnvVar::list_by_project_id(pool, project.id).await?;
    let configured_names: std::collections::HashSet<String> =
        configured_vars.into_iter().map(|v| v.name).collect();

    // Build response
    let env_vars: Vec<EnvVarEntry> = allowed_names
        .into_iter()
        .map(|name| {
            let configured = configured_names.contains(&name);
            EnvVarEntry { name, configured }
        })
        .collect();

    Ok(Json(ApiResponse::success(ProjectEnvVarsResponse {
        env_vars,
    })))
}

/// Update environment variables for a project.
///
/// Only variables in the vibekanban.json allowlist can be set.
pub async fn update_project_env_vars(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<UpdateProjectEnvVarsRequest>,
) -> Result<Json<ApiResponse<ProjectEnvVarsResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    // Get project repositories
    let repos = deployment
        .project()
        .get_repositories(pool, project.id)
        .await?;

    // Collect allowed env var names from vibekanban.json files
    let allowed_names = collect_env_vars_for_repos(&repos);
    let allowed_set: std::collections::HashSet<&String> = allowed_names.iter().collect();

    // Process set operations
    if let Some(set_values) = payload.set {
        for (name, value) in set_values {
            if allowed_set.contains(&name) {
                ProjectEnvVar::upsert(pool, project.id, &name, &value).await?;
            }
            // Silently ignore names not in the allowlist
        }
    }

    // Process clear operations
    if let Some(clear_names) = payload.clear {
        for name in clear_names {
            if allowed_set.contains(&name) {
                ProjectEnvVar::delete(pool, project.id, &name).await?;
            }
            // Silently ignore names not in the allowlist
        }
    }

    // Return updated state
    let configured_vars = ProjectEnvVar::list_by_project_id(pool, project.id).await?;
    let configured_names: std::collections::HashSet<String> =
        configured_vars.into_iter().map(|v| v.name).collect();

    let env_vars: Vec<EnvVarEntry> = allowed_names
        .into_iter()
        .map(|name| {
            let configured = configured_names.contains(&name);
            EnvVarEntry { name, configured }
        })
        .collect();

    Ok(Json(ApiResponse::success(ProjectEnvVarsResponse {
        env_vars,
    })))
}

pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new().route("/", get(get_project_env_vars).put(update_project_env_vars))
}
