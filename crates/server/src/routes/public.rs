use axum::{Router, extract::{Path, State}, http::StatusCode, response::Json, routing::get};
use chrono::{DateTime, Utc};
use db::models::{inbox_item::InboxItem, project::Project, task::{Task, TaskStatus}};
use deployment::Deployment;
use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

use crate::DeploymentImpl;
use crate::routes::tasks::WorkItemsResponse;
use utils::response::ApiResponse;

/// Task information within a pipeline
#[derive(Debug, Serialize, TS)]
pub struct PublicTask {
    pub id: Uuid,
    pub title: String,
    /// How long the task has been in the current state (in seconds)
    pub time_in_state_seconds: i64,
    /// When the task entered its current state
    #[ts(type = "Date")]
    pub state_since: DateTime<Utc>,
}

/// Pipeline (status) with its tasks
#[derive(Debug, Serialize, TS)]
pub struct PublicPipeline {
    pub status: String,
    pub task_count: usize,
    pub tasks: Vec<PublicTask>,
}

/// Project with pipeline status information
#[derive(Debug, Serialize, TS)]
pub struct PublicProject {
    pub id: Uuid,
    pub name: String,
    pub pipelines: Vec<PublicPipeline>,
}

/// Build pipelines from tasks, grouping by status
fn build_pipelines(tasks: Vec<Task>) -> Vec<PublicPipeline> {
    let now = Utc::now();
    
    // Define all possible statuses in order
    let statuses = [
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::Done,
        TaskStatus::Cancelled,
    ];
    
    statuses
        .into_iter()
        .map(|status| {
            let status_tasks: Vec<PublicTask> = tasks
                .iter()
                .filter(|t| t.status == status)
                .map(|t| {
                    let time_in_state = now.signed_duration_since(t.updated_at);
                    PublicTask {
                        id: t.id,
                        title: t.title.clone(),
                        time_in_state_seconds: time_in_state.num_seconds(),
                        state_since: t.updated_at,
                    }
                })
                .collect();
            
            PublicPipeline {
                status: status.to_string(),
                task_count: status_tasks.len(),
                tasks: status_tasks,
            }
        })
        .collect()
}

/// List all available projects with pipeline status information
pub async fn list_projects(
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<ApiResponse<Vec<PublicProject>>>, StatusCode> {
    let pool = &deployment.db().pool;
    
    // Fetch all projects
    let projects = match Project::find_all(pool).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to fetch projects for public API: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Build public projects with pipeline information
    let mut public_projects = Vec::with_capacity(projects.len());
    
    for project in projects {
        // Fetch tasks for this project using the Task model method
        let tasks = match Task::find_by_project_id(pool, project.id).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(
                    "Failed to fetch tasks for project {} in public API: {}",
                    project.id,
                    e
                );
                // Continue with empty tasks for this project
                Vec::new()
            }
        };
        
        let pipelines = build_pipelines(tasks);
        
        public_projects.push(PublicProject {
            id: project.id,
            name: project.name,
            pipelines,
        });
    }
    
    Ok(Json(ApiResponse::success(public_projects)))
}

/// Get all inbox items and active tasks (not Done/Cancelled) for a project - no auth required
pub async fn get_work_items(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ApiResponse<WorkItemsResponse>>, StatusCode> {
    let pool = &deployment.db().pool;

    let inbox_items = InboxItem::list_by_project(pool, project_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch inbox items for project {}: {}", project_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let tasks = Task::find_active_by_project_id_with_attempt_status(pool, project_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch active tasks for project {}: {}", project_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(ApiResponse::success(WorkItemsResponse {
        inbox_items,
        tasks,
    })))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/projects", get(list_projects))
        .route("/projects/{project_id}/work-items", get(get_work_items))
}
