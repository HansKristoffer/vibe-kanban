use std::path::Path as StdPath;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    Extension,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use db::models::inbox_item::{
    CreateInboxItem, InboxItem, InboxItemKind, InboxItemStatus, InboxSource, UpdateInboxItem,
};
use db::models::project_integrations::ProjectIntegrations;
use db::models::project_member::ProjectMember;
use db::models::project_repo::ProjectRepo;
use db::models::task::{CreateTask, Task};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::AuthenticatedUser};
use deployment::Deployment;
use services::services::claude_code_prd::{
    ClaudeCodePrdService, ClaudeCodePrdError, DEFAULT_INBOX_PRD_TEMPLATE,
};
use services::services::inbox_integrations::linear_create_issue;
use utils::response::ApiResponse;

#[derive(Debug, Deserialize, TS)]
pub struct InboxQuery {
    pub project_id: Uuid,
    pub status: Option<InboxItemStatus>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateInboxItemRequest {
    pub project_id: Uuid,
    pub title: String,
    pub body: String,
    pub source_url: Option<String>,
    /// Whether to generate a PRD from the body using AI. Defaults to true.
    pub generate_prd: Option<bool>,
}

#[derive(Debug, Serialize, TS)]
pub struct AcceptInboxResponse {
    pub task_id: Uuid,
}

#[derive(Debug, Deserialize, TS)]
pub struct UpdateInboxItemRequest {
    pub title: Option<String>,
    pub prd_markdown: Option<String>,
}

/// Returns (kind, prd_markdown, actionable, recommend_ralph)
async fn classify_prd(
    repo_path: &StdPath,
    input: &str,
    prd_template: &str,
) -> Result<(InboxItemKind, String, bool, bool), ClaudeCodePrdError> {
    let result = ClaudeCodePrdService::classify_and_generate_prd(repo_path, input, prd_template)
        .await?;
    Ok((result.kind, result.prd_markdown, result.actionable, result.recommend_ralph))
}

pub async fn list_inbox_items(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<InboxQuery>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<Vec<InboxItem>>>, ApiError> {
    let pool = &deployment.db().pool;
    ensure_project_member(pool, query.project_id, &user.email).await?;
    let items = if let Some(status) = query.status {
        InboxItem::list_by_project_and_status(pool, query.project_id, status).await?
    } else {
        InboxItem::list_by_project(pool, query.project_id).await?
    };
    Ok(Json(ApiResponse::success(items)))
}

pub async fn get_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Path(inbox_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let item = InboxItem::find_by_id(&deployment.db().pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;
    ensure_project_member(&deployment.db().pool, item.project_id, &user.email).await?;
    Ok(Json(ApiResponse::success(item)))
}

pub async fn create_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<CreateInboxItemRequest>,
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let pool = &deployment.db().pool;
    ensure_project_member(pool, payload.project_id, &user.email).await?;
    
    let project = db::models::project::Project::find_by_id(pool, payload.project_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project not found".to_string()))?;
    
    // Get the project's repository path for Claude Code context
    let repos = ProjectRepo::find_repos_for_project(pool, payload.project_id).await?;
    let repo = repos
        .first()
        .ok_or_else(|| ApiError::BadRequest("No repository configured for project. Claude Code requires a repository to analyze.".to_string()))?;
    let repo_path_string = repo.path.to_string_lossy().to_string();
    
    let prd_template = project
        .inbox_prd_template
        .as_deref()
        .filter(|template| !template.trim().is_empty())
        .unwrap_or(DEFAULT_INBOX_PRD_TEMPLATE)
        .to_string();
    let action_token = Uuid::new_v4().to_string();
    let source_item_id = Uuid::new_v4().to_string();
    
    // Check if PRD generation is enabled (defaults to true)
    let should_generate_prd = payload.generate_prd.unwrap_or(true);
    
    let raw_payload_json = serde_json::json!({
        "title": payload.title,
        "body": payload.body,
        "source_url": payload.source_url,
        "generate_prd": should_generate_prd,
    })
    .to_string();

    // If not generating PRD, use body directly
    let (kind, prd_markdown) = if !should_generate_prd {
        (InboxItemKind::Other, Some(payload.body.clone()))
    } else {
        // PRD will be generated in background, start with no PRD
        (InboxItemKind::Other, None)
    };

    let item = InboxItem::create(
        pool,
        &CreateInboxItem {
            project_id: payload.project_id,
            source: InboxSource::Manual,
            source_item_id,
            source_url: payload.source_url,
            title: payload.title.clone(),
            raw_payload_json: Some(raw_payload_json),
            kind,
            status: InboxItemStatus::Pending,
            prd_markdown,
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    // Spawn background task to generate PRD if enabled
    if should_generate_prd {
        let item_id = item.id;
        let body = payload.body.clone();
        tokio::spawn(async move {
            generate_prd_background(deployment, item_id, repo_path_string, body, prd_template).await;
        });
    }

    Ok(Json(ApiResponse::success(item)))
}

/// Background task to generate PRD and update the inbox item
async fn generate_prd_background(
    deployment: DeploymentImpl,
    item_id: Uuid,
    repo_path_string: String,
    body: String,
    prd_template: String,
) {
    info!("Starting PRD generation for inbox item {}", item_id);
    let repo_path = StdPath::new(&repo_path_string);
    
    match classify_prd(repo_path, &body, &prd_template).await {
        Ok((kind, prd_markdown, _actionable, recommend_ralph)) => {
            info!("PRD generated successfully for inbox item {}, kind: {:?}, recommend_ralph: {}", item_id, kind, recommend_ralph);
            
            // Add Ralph recommendation note at the top of PRD if recommended
            let final_prd = if recommend_ralph {
                format!(
                    "> **Recommended: Use Ralph mode** - This task is complex and would benefit from being split into multiple implementation steps. Enable Ralph when starting the task to have each checklist item implemented in a separate session.\n\n{}",
                    prd_markdown
                )
            } else {
                prd_markdown
            };
            
            // Update the inbox item with the generated PRD
            if let Err(e) = InboxItem::update(
                &deployment.db().pool,
                item_id,
                &UpdateInboxItem {
                    title: None,
                    kind: Some(kind),
                    status: None,
                    prd_markdown: Some(final_prd),
                    task_id: None,
                    linear_issue_id: None,
                    linear_issue_url: None,
                    outbound_last_error: None,
                },
            )
            .await
            {
                warn!("Failed to update inbox item {} with PRD: {}", item_id, e);
            } else {
                info!("Inbox item {} updated with generated PRD", item_id);
            }
        }
        Err(e) => {
            warn!("Claude Code PRD generation failed for inbox item {}: {}", item_id, e);
            // Item remains with no PRD, user can manually edit or retry later
        }
    }
}

pub async fn update_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Path(inbox_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<UpdateInboxItemRequest>,
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_id(pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;
    ensure_project_member(pool, item.project_id, &user.email).await?;

    let updated = InboxItem::update(
        pool,
        item.id,
        &UpdateInboxItem {
            title: payload.title,
            kind: None,
            status: None,
            prd_markdown: payload.prd_markdown,
            task_id: None,
            linear_issue_id: None,
            linear_issue_url: None,
            outbound_last_error: None,
        },
    )
    .await?;

    Ok(Json(ApiResponse::success(updated)))
}

pub async fn accept_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Path(inbox_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<AcceptInboxResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_id(pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;
    ensure_project_member(pool, item.project_id, &user.email).await?;

    let response = accept_inbox_item_internal(pool, &item).await?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn decline_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Path(inbox_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_id(pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;
    ensure_project_member(pool, item.project_id, &user.email).await?;

    let updated = decline_inbox_item_internal(pool, &item).await?;
    Ok(Json(ApiResponse::success(updated)))
}

async fn accept_inbox_item_internal(
    pool: &sqlx::SqlitePool,
    item: &InboxItem,
) -> Result<AcceptInboxResponse, ApiError> {
    if matches!(item.status, InboxItemStatus::Accepted) {
        let task_id = item
            .task_id
            .ok_or_else(|| ApiError::BadRequest("Inbox item is accepted without task".to_string()))?;
        return Ok(AcceptInboxResponse { task_id });
    }

    // 1. Create task first (so we have task_id for Linear issue)
    let task = Task::create(
        pool,
        &CreateTask::from_title_description(
            item.project_id,
            item.title.clone(),
            item.prd_markdown.clone(),
        ),
        Uuid::new_v4(),
    )
    .await?;

    // 2. Try to create Linear issue if Linear is configured and source is not Linear
    // Linear is optional - if it fails or isn't configured, we still create the task
    let mut linear_issue_id = item.linear_issue_id.clone();
    let mut linear_issue_url = item.linear_issue_url.clone();

    if !matches!(item.source, InboxSource::Linear) && linear_issue_id.is_none() {
        // Try to create Linear issue, but don't fail if Linear isn't configured or fails
        if let Ok(Some(integrations)) = ProjectIntegrations::find_by_project_id(pool, item.project_id).await {
            if let (Some(api_key), Some(team_id)) = (
                integrations.linear_api_key.as_deref(),
                integrations.linear_team_id.as_deref(),
            ) {
                let mut description = item.prd_markdown.clone().unwrap_or_default();

                // Add VK task link at the top of description
                let vk_task_url = services::services::slack::get_vk_task_url(&item.project_id, &task.id);
                if !vk_task_url.is_empty() {
                    let vk_link = format!("**[View in Vibe Kanban]({})**\n\n", vk_task_url);
                    description = format!("{}{}", vk_link, description);
                }

                if let Some(source_url) = item.source_url.as_ref() {
                    if !description.trim().is_empty() {
                        description.push_str("\n\n");
                    }
                    description.push_str(&format!("Source: {}", source_url));
                }
                if description.trim().is_empty() {
                    description = item.title.clone();
                }

                match linear_create_issue(
                    api_key,
                    team_id,
                    &item.title,
                    &description,
                    integrations.linear_state_id_todo.as_deref(),
                )
                .await
                {
                    Ok(issue) => {
                        linear_issue_id = Some(issue.id);
                        linear_issue_url = issue.url;
                    }
                    Err(e) => {
                        warn!("Failed to create Linear issue for inbox item {}: {}", item.id, e);
                    }
                }
            }
        }
    }

    InboxItem::update(
        pool,
        item.id,
        &UpdateInboxItem {
            title: None,
            kind: None,
            status: Some(InboxItemStatus::Accepted),
            prd_markdown: None,
            task_id: Some(task.id),
            linear_issue_id,
            linear_issue_url,
            outbound_last_error: None,
        },
    )
    .await?;

    Ok(AcceptInboxResponse { task_id: task.id })
}

async fn decline_inbox_item_internal(
    pool: &sqlx::SqlitePool,
    item: &InboxItem,
) -> Result<InboxItem, ApiError> {
    let updated = InboxItem::update(
        pool,
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
    .await?;

    Ok(updated)
}

async fn ensure_project_member(
    pool: &sqlx::SqlitePool,
    project_id: Uuid,
    email: &str,
) -> Result<(), ApiError> {
    // Skip membership check when auth is disabled
    if std::env::var("AUTH_DISABLED")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return Ok(());
    }

    match ProjectMember::is_member(pool, project_id, email).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiError::Forbidden("Access denied".to_string())),
        Err(err) => Err(ApiError::Database(err)),
    }
}

pub async fn accept_inbox_item_action(
    State(deployment): State<DeploymentImpl>,
    Path(action_token): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_action_token(pool, &action_token)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

    let _ = accept_inbox_item_internal(pool, &item).await?;
    let refreshed = InboxItem::find_by_id(pool, item.id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;
    let task_id = refreshed.task_id.ok_or_else(|| {
        ApiError::BadRequest("Inbox item missing task after accept".to_string())
    })?;
    let redirect = format!("/projects/{}/tasks/{}", refreshed.project_id, task_id);
    Ok(Redirect::to(&redirect))
}

pub async fn decline_inbox_item_action(
    State(deployment): State<DeploymentImpl>,
    Path(action_token): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_action_token(pool, &action_token)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

    let _ = decline_inbox_item_internal(pool, &item).await?;
    let redirect = format!("/projects/{}/inbox", item.project_id);
    Ok(Redirect::to(&redirect))
}

pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/", get(list_inbox_items).post(create_inbox_item))
        .route("/{inbox_id}", get(get_inbox_item).put(update_inbox_item))
        .route("/{inbox_id}/accept", post(accept_inbox_item))
        .route("/{inbox_id}/decline", post(decline_inbox_item))
        .route("/action/{action_token}/accept", get(accept_inbox_item_action))
        .route("/action/{action_token}/decline", get(decline_inbox_item_action))
}
