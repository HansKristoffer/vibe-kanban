use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use db::models::inbox_item::{
    CreateInboxItem, InboxItem, InboxItemKind, InboxItemStatus, InboxSource, UpdateInboxItem,
};
use db::models::project_integrations::ProjectIntegrations;
use db::models::task::{CreateTask, Task};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};
use deployment::Deployment;
use services::services::anthropic::{
    AnthropicClient, DEFAULT_INBOX_PRD_TEMPLATE,
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

async fn classify_prd(
    input: &str,
    prd_template: &str,
) -> Option<(InboxItemKind, String, bool)> {
    let client = AnthropicClient::from_env().ok()?;
    let result = client
        .classify_and_generate_prd_with_template(input, prd_template)
        .await
        .ok()?;
    Some((result.kind, result.prd_markdown, result.actionable))
}

pub async fn list_inbox_items(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<ApiResponse<Vec<InboxItem>>>, ApiError> {
    let pool = &deployment.db().pool;
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
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let item = InboxItem::find_by_id(&deployment.db().pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;
    Ok(Json(ApiResponse::success(item)))
}

pub async fn create_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateInboxItemRequest>,
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let project = db::models::project::Project::find_by_id(
        &deployment.db().pool,
        payload.project_id,
    )
    .await?
    .ok_or_else(|| ApiError::BadRequest("Project not found".to_string()))?;
    let prd_template = project
        .inbox_prd_template
        .as_deref()
        .filter(|template| !template.trim().is_empty())
        .unwrap_or(DEFAULT_INBOX_PRD_TEMPLATE);
    let action_token = Uuid::new_v4().to_string();
    let source_item_id = Uuid::new_v4().to_string();
    let raw_payload_json = serde_json::json!({
        "title": payload.title,
        "body": payload.body,
        "source_url": payload.source_url,
    })
    .to_string();

    let (kind, prd_markdown, _actionable) =
        classify_prd(&payload.body, prd_template)
        .await
        .map(|(kind, prd, actionable)| (kind, prd, actionable))
        .unwrap_or((InboxItemKind::Other, payload.body.clone(), true));

    let item = InboxItem::create(
        &deployment.db().pool,
        &CreateInboxItem {
            project_id: payload.project_id,
            source: InboxSource::Manual,
            source_item_id,
            source_url: payload.source_url,
            title: payload.title,
            raw_payload_json: Some(raw_payload_json),
            kind,
            status: InboxItemStatus::Pending,
            prd_markdown: Some(prd_markdown),
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    Ok(Json(ApiResponse::success(item)))
}

pub async fn update_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Path(inbox_id): Path<Uuid>,
    Json(payload): Json<UpdateInboxItemRequest>,
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_id(pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

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
) -> Result<Json<ApiResponse<AcceptInboxResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_id(pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

    if matches!(item.status, InboxItemStatus::Accepted) {
        let task_id = item
            .task_id
            .ok_or_else(|| ApiError::BadRequest("Inbox item is accepted without task".to_string()))?;
        return Ok(Json(ApiResponse::success(AcceptInboxResponse { task_id })));
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

    // 2. Create Linear issue if Linear is configured and source is not Linear
    let mut linear_issue_id = item.linear_issue_id.clone();
    let mut linear_issue_url = item.linear_issue_url.clone();

    if !matches!(item.source, InboxSource::Linear) && linear_issue_id.is_none() {
        let integrations = ProjectIntegrations::find_by_project_id(pool, item.project_id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Linear integration not configured".to_string()))?;
        let api_key = integrations
            .linear_api_key
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("Linear API key not configured".to_string()))?;
        let team_id = integrations
            .linear_team_id
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("Linear team not configured".to_string()))?;

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

        let issue = linear_create_issue(
            api_key,
            team_id,
            &item.title,
            &description,
            integrations.linear_state_id_todo.as_deref(),
        )
        .await
        .map_err(|err| ApiError::BadRequest(format!("Linear issue create failed: {}", err)))?;

        linear_issue_id = Some(issue.id);
        linear_issue_url = issue.url;
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

    Ok(Json(ApiResponse::success(AcceptInboxResponse { task_id: task.id })))
}

pub async fn decline_inbox_item(
    State(deployment): State<DeploymentImpl>,
    Path(inbox_id): Path<Uuid>,
) -> Result<Json<ApiResponse<InboxItem>>, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_id(pool, inbox_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

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

    Ok(Json(ApiResponse::success(updated)))
}

pub async fn accept_inbox_item_action(
    State(deployment): State<DeploymentImpl>,
    Path(action_token): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let pool = &deployment.db().pool;
    let item = InboxItem::find_by_action_token(pool, &action_token)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

    let _ = accept_inbox_item(State(deployment.clone()), Path(item.id)).await?;
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

    let _ = decline_inbox_item(State(deployment), Path(item.id)).await?;
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
