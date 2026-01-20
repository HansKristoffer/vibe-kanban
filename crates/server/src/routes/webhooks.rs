use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::post,
};
use axum::body::Bytes;
use db::models::inbox_item::{CreateInboxItem, InboxItem, InboxItemKind, InboxItemStatus, InboxSource, UpsertInboxItem};
use db::models::project::Project;
use db::models::project_integrations::ProjectIntegrations;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};
use deployment::Deployment;
use services::services::anthropic::{
    AnthropicClient, AnthropicInboxResult, DEFAULT_INBOX_PRD_TEMPLATE,
};
use services::services::inbox_outbound::post_registered_if_needed;
use services::services::posthog_sentry_enrichment::{
    fetch_posthog_event, fetch_sentry_issue,
};
use services::services::container::ContainerService;
use services::services::slack::{
    self, build_prd_blocks_json, build_accept_modal_json,
    build_task_accepted_message_json, get_task_accepted_text, get_vk_task_url,
    PrdMessageStatus, SlackClient,
};
use utils::response::ApiResponse;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize)]
struct ManualWebhookPayload {
    source_item_id: String,
    title: String,
    body: String,
    source_url: Option<String>,
    kind: Option<InboxItemKind>,
    force_pending: Option<bool>,
}

/// Request payload for the personal-ai quick endpoint.
/// This endpoint creates a PRD, posts to Slack, auto-accepts, and starts Claude Code immediately.
#[derive(Debug, Deserialize)]
pub struct PersonalAiPayload {
    /// The idea/description from your personal AI
    text: String,
    /// Override the title (otherwise derived from LLM output)
    title: Option<String>,
    /// Idempotency key (fallback: random UUID)
    source_item_id: Option<String>,
    /// Context link
    source_url: Option<String>,
    /// Base branch for workspace repos (default: "main")
    base_branch: Option<String>,
    /// Slack user ID to mention/tag and store for notifications
    slack_user_id: Option<String>,
}

/// Response for the personal-ai quick endpoint.
#[derive(Debug, Serialize)]
pub struct PersonalAiResponse {
    inbox_item_id: Uuid,
    task_id: Uuid,
    task_url: String,
    workspace_id: Option<Uuid>,
    execution_process_id: Option<Uuid>,
    slack_posted: bool,
    slack_channel_id: Option<String>,
    slack_message_ts: Option<String>,
    started: bool,
    start_error: Option<String>,
}

fn verify_hmac_sha256(secret: &str, signature_header: &str, payload: &[u8]) -> bool {
    let signature = signature_header
        .strip_prefix("sha256=")
        .unwrap_or(signature_header);
    let Ok(expected_signature) = hex::decode(signature) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(payload);
    let computed_signature = mac.finalize().into_bytes();
    computed_signature[..].ct_eq(&expected_signature).into()
}

fn extract_string(payload: &serde_json::Value, pointers: &[&str]) -> Option<String> {
    for pointer in pointers {
        if let Some(value) = payload.pointer(pointer).and_then(|v| v.as_str()) {
            return Some(value.to_string());
        }
    }
    None
}

fn extract_text(payload: &serde_json::Value, pointers: &[&str]) -> Option<String> {
    for pointer in pointers {
        if let Some(value) = payload.pointer(pointer) {
            if let Some(text) = value.as_str() {
                return Some(text.to_string());
            }
            if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
        }
    }
    None
}

async fn load_project_integrations(
    deployment: &DeploymentImpl,
    webhook_token: &str,
) -> Result<ProjectIntegrations, ApiError> {
    let integrations =
        ProjectIntegrations::find_by_webhook_token(&deployment.db().pool, webhook_token)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Unknown webhook token".to_string()))?;
    Ok(integrations)
}

async fn load_project(
    deployment: &DeploymentImpl,
    project_id: Uuid,
) -> Result<Project, ApiError> {
    Project::find_by_id(&deployment.db().pool, project_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project not found".to_string()))
}

async fn upsert_inbox_item(
    deployment: &DeploymentImpl,
    data: UpsertInboxItem,
    id: Uuid,
) -> Result<InboxItem, ApiError> {
    InboxItem::upsert_by_source(&deployment.db().pool, &data, id)
        .await
        .map_err(ApiError::from)
}

fn effective_prd_template(project: &Project) -> &str {
    project
        .inbox_prd_template
        .as_deref()
        .filter(|template| !template.trim().is_empty())
        .unwrap_or(DEFAULT_INBOX_PRD_TEMPLATE)
}

async fn classify_payload(
    input: &str,
    prd_template: &str,
) -> Option<AnthropicInboxResult> {
    let client = AnthropicClient::from_env().ok()?;
    match client
        .classify_and_generate_prd_with_template(input, prd_template)
        .await
    {
        Ok(result) => Some(result),
        Err(err) => {
            warn!("Anthropic classification failed: {}", err);
            None
        }
    }
}

/// Post a PRD to Slack if Slack integration is configured.
/// Returns Ok(()) even if posting fails (to not block the webhook response).
async fn post_prd_to_slack_if_configured(
    deployment: &DeploymentImpl,
    integrations: &ProjectIntegrations,
    item: &InboxItem,
) {
    // Only post to Slack if:
    // 1. Slack is configured (bot_token and channel_id)
    // 2. Item status is Pending (actionable)
    // 3. Item is not already posted to Slack (slack_message_ts is None)
    if item.slack_message_ts.is_some() {
        return;
    }
    
    let bot_token = match integrations.slack_bot_token.as_ref() {
        Some(t) => t,
        None => return,
    };
    
    let channel_id = match integrations.slack_channel_id.as_ref() {
        Some(c) => c,
        None => return,
    };

    // Only post pending items
    if !matches!(item.status, InboxItemStatus::Pending) {
        return;
    }

    let client = SlackClient::new(bot_token);
    let kind_str = format!("{:?}", item.kind).to_lowercase();
    let source_str = format!("{:?}", item.source).to_lowercase();
    let prd = item.prd_markdown.as_deref().unwrap_or("");
    
    let blocks = build_prd_blocks_json(
        &item.title,
        &kind_str,
        &source_str,
        prd,
        &item.id.to_string(),
        PrdMessageStatus::Pending,
    );

    match client.post_message_json(channel_id, &item.title, blocks, None).await {
        Ok(result) => {
            // Store the Slack message reference
            if let Err(e) = InboxItem::set_slack_message(
                &deployment.db().pool,
                item.id,
                &result.channel,
                &result.ts,
            ).await {
                warn!("Failed to store Slack message reference: {}", e);
            } else {
                info!("Posted PRD to Slack: {} (ts: {})", item.title, result.ts);
            }
        }
        Err(e) => {
            warn!("Failed to post PRD to Slack: {}", e);
        }
    }
}

pub async fn linear_webhook(
    State(deployment): State<DeploymentImpl>,
    Path(webhook_token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let integrations = load_project_integrations(&deployment, &webhook_token).await?;
    if let Some(secret) = integrations.linear_webhook_secret.as_ref() {
        let signature = headers
            .get("Linear-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized)?;
        if !verify_hmac_sha256(secret, signature, &body) {
            return Err(ApiError::Unauthorized);
        }
    }

    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let source_item_id = extract_string(
        &payload,
        &["/data/issue/id", "/data/id", "/issue/id", "/id"],
    )
    .ok_or_else(|| ApiError::BadRequest("Missing Linear issue id".to_string()))?;
    let title = extract_text(
        &payload,
        &["/data/issue/title", "/data/title", "/issue/title", "/title"],
    )
    .unwrap_or_else(|| "Linear issue".to_string());
    let description = extract_text(
        &payload,
        &["/data/issue/description", "/data/description", "/issue/description", "/description"],
    );
    let source_url = extract_string(
        &payload,
        &["/data/issue/url", "/data/url", "/issue/url", "/url"],
    );

    let project = load_project(&deployment, integrations.project_id).await?;
    let prd_template = effective_prd_template(&project);
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let mut enrichment_text = raw_payload_json.clone();
    if let (Some(api_key), Some(host), Some(project_id)) = (
        integrations.posthog_api_key.as_deref(),
        integrations.posthog_host.as_deref(),
        integrations.posthog_project_id.as_deref(),
    ) {
        if let Ok(enrichment) = fetch_posthog_event(api_key, host, project_id, &source_item_id).await
        {
            enrichment_text = format!(
                "PostHog event: {}\nURL: {}\nDetails: {}\n\nRaw payload:\n{}",
                enrichment.title,
                enrichment.url.clone().unwrap_or_else(|| "N/A".to_string()),
                enrichment.description,
                raw_payload_json
            );
        }
    }

    let classification_input = if let Some(ref desc) = description {
        format!("Title: {}\n\nDescription:\n{}\n\nPayload:\n{}", title, desc, enrichment_text)
    } else {
        format!("Title: {}\n\nPayload:\n{}", title, enrichment_text)
    };
    let classification = classify_payload(&classification_input, prd_template).await;
    let (kind, status, prd_markdown) = match classification {
        Some(result) if result.actionable && !matches!(result.kind, InboxItemKind::Other) => {
            (result.kind, InboxItemStatus::Pending, Some(result.prd_markdown))
        }
        Some(result) => (result.kind, InboxItemStatus::Ignored, Some(result.prd_markdown)),
        None => (InboxItemKind::Feature, InboxItemStatus::Pending, description),
    };

    let item = upsert_inbox_item(
        &deployment,
        UpsertInboxItem {
            project_id: integrations.project_id,
            source: InboxSource::Linear,
            source_item_id,
            source_url,
            title,
            raw_payload_json: Some(raw_payload_json),
            kind,
            status,
            prd_markdown,
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    post_registered_if_needed(&deployment.db().pool, &integrations, &item).await;

    Ok(Json(ApiResponse::success(())))
}

pub async fn intercom_webhook(
    State(deployment): State<DeploymentImpl>,
    Path(webhook_token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let integrations = load_project_integrations(&deployment, &webhook_token).await?;
    if let Some(secret) = integrations.intercom_webhook_secret.as_ref() {
        let signature = headers
            .get("X-Intercom-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized)?;
        if !verify_hmac_sha256(secret, signature, &body) {
            return Err(ApiError::Unauthorized);
        }
    }

    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let source_item_id = extract_string(&payload, &["/data/item/id", "/data/id", "/id"])
        .ok_or_else(|| ApiError::BadRequest("Missing Intercom conversation id".to_string()))?;
    let title = extract_text(
        &payload,
        &[
            "/data/item/title",
            "/data/item/subject",
            "/data/item/conversation_message/subject",
            "/data/item/conversation_message/body",
        ],
    )
    .unwrap_or_else(|| "Intercom conversation".to_string());
    let source_url = extract_string(&payload, &["/data/item/url", "/data/url", "/url"]);

    let project = load_project(&deployment, integrations.project_id).await?;
    let prd_template = effective_prd_template(&project);
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let mut enrichment_text = raw_payload_json.clone();
    if let (Some(api_token), Some(org_slug), Some(project_slug)) = (
        integrations.sentry_api_token.as_deref(),
        integrations.sentry_org_slug.as_deref(),
        integrations.sentry_project_slug.as_deref(),
    ) {
        if let Ok(enrichment) =
            fetch_sentry_issue(api_token, org_slug, project_slug, &source_item_id).await
        {
            let stacktrace = enrichment
                .stacktrace
                .unwrap_or_else(|| "N/A".to_string());
            enrichment_text = format!(
                "Sentry issue: {}\nCulprit: {}\nLevel: {}\nURL: {}\nStacktrace: {}\n\nRaw payload:\n{}",
                enrichment.title,
                enrichment.culprit.unwrap_or_else(|| "N/A".to_string()),
                enrichment.level.unwrap_or_else(|| "N/A".to_string()),
                enrichment.url.unwrap_or_else(|| "N/A".to_string()),
                stacktrace,
                raw_payload_json
            );
        }
    }

    let classification = classify_payload(
        &format!("Title: {}\n\nPayload:\n{}", title, enrichment_text),
        prd_template,
    )
    .await;
    let (kind, status, prd_markdown) = match classification {
        Some(result) if result.actionable && !matches!(result.kind, InboxItemKind::Other) => {
            (result.kind, InboxItemStatus::Pending, Some(result.prd_markdown))
        }
        Some(result) => (result.kind, InboxItemStatus::Ignored, Some(result.prd_markdown)),
        None => (InboxItemKind::Other, InboxItemStatus::Pending, None),
    };

    let item = upsert_inbox_item(
        &deployment,
        UpsertInboxItem {
            project_id: integrations.project_id,
            source: InboxSource::Intercom,
            source_item_id,
            source_url,
            title,
            raw_payload_json: Some(raw_payload_json),
            kind,
            status,
            prd_markdown,
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    post_registered_if_needed(&deployment.db().pool, &integrations, &item).await;
    // Post to Slack if configured
    post_prd_to_slack_if_configured(&deployment, &integrations, &item).await;

    Ok(Json(ApiResponse::success(())))
}

pub async fn modjo_webhook(
    State(deployment): State<DeploymentImpl>,
    Path(webhook_token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let integrations = load_project_integrations(&deployment, &webhook_token).await?;
    if let Some(secret) = integrations.modjo_webhook_secret.as_ref() {
        let signature = headers
            .get("X-Modjo-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized)?;
        if !verify_hmac_sha256(secret, signature, &body) {
            return Err(ApiError::Unauthorized);
        }
    }

    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let source_item_id = extract_string(&payload, &["/data/id", "/id"])
        .ok_or_else(|| ApiError::BadRequest("Missing Modjo item id".to_string()))?;
    let title = extract_text(&payload, &["/data/title", "/data/name", "/title"])
        .unwrap_or_else(|| "Modjo item".to_string());
    let source_url = extract_string(&payload, &["/data/url", "/url"]);

    let project = load_project(&deployment, integrations.project_id).await?;
    let prd_template = effective_prd_template(&project);
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(
        &format!("Title: {}\n\nPayload:\n{}", title, raw_payload_json),
        prd_template,
    )
    .await;
    let (kind, status, prd_markdown) = match classification {
        Some(result) if result.actionable && !matches!(result.kind, InboxItemKind::Other) => {
            (result.kind, InboxItemStatus::Pending, Some(result.prd_markdown))
        }
        Some(result) => (result.kind, InboxItemStatus::Ignored, Some(result.prd_markdown)),
        None => (InboxItemKind::Other, InboxItemStatus::Pending, None),
    };

    let item = upsert_inbox_item(
        &deployment,
        UpsertInboxItem {
            project_id: integrations.project_id,
            source: InboxSource::Modjo,
            source_item_id,
            source_url,
            title,
            raw_payload_json: Some(raw_payload_json),
            kind,
            status,
            prd_markdown,
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    // Post to Slack if configured
    post_prd_to_slack_if_configured(&deployment, &integrations, &item).await;

    Ok(Json(ApiResponse::success(())))
}

pub async fn manual_webhook(
    State(deployment): State<DeploymentImpl>,
    Path(webhook_token): Path<String>,
    body: Bytes,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let integrations = load_project_integrations(&deployment, &webhook_token).await?;
    let payload: ManualWebhookPayload =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let project = load_project(&deployment, integrations.project_id).await?;
    let prd_template = effective_prd_template(&project);
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(&payload.body, prd_template).await;
    let mut kind = payload.kind.unwrap_or(InboxItemKind::Other);
    let mut status = InboxItemStatus::Pending;
    let mut prd_markdown = Some(payload.body);

    if let Some(result) = classification {
        kind = result.kind.clone();
        prd_markdown = Some(result.prd_markdown);
        status = if result.actionable && !matches!(result.kind, InboxItemKind::Other) {
            InboxItemStatus::Pending
        } else {
            InboxItemStatus::Ignored
        };
    }

    if payload.force_pending.unwrap_or(false) {
        status = InboxItemStatus::Pending;
    }

    let item = upsert_inbox_item(
        &deployment,
        UpsertInboxItem {
            project_id: integrations.project_id,
            source: InboxSource::Manual,
            source_item_id: payload.source_item_id,
            source_url: payload.source_url,
            title: payload.title,
            raw_payload_json: Some(raw_payload_json),
            kind,
            status,
            prd_markdown,
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    // Post to Slack if configured
    post_prd_to_slack_if_configured(&deployment, &integrations, &item).await;

    Ok(Json(ApiResponse::success(())))
}

pub async fn posthog_webhook(
    State(deployment): State<DeploymentImpl>,
    Path(webhook_token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let integrations = load_project_integrations(&deployment, &webhook_token).await?;
    if let Some(secret) = integrations.posthog_webhook_secret.as_ref() {
        let signature = headers
            .get("X-Posthog-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized)?;
        if !verify_hmac_sha256(secret, signature, &body) {
            return Err(ApiError::Unauthorized);
        }
    }

    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let source_item_id = extract_string(
        &payload,
        &[
            "/data/event/uuid",
            "/data/uuid",
            "/event/uuid",
            "/uuid",
            "/id",
        ],
    )
    .ok_or_else(|| ApiError::BadRequest("Missing PostHog event id".to_string()))?;
    let title = extract_text(
        &payload,
        &[
            "/data/event/name",
            "/data/event/message",
            "/data/name",
            "/data/message",
            "/name",
            "/message",
        ],
    )
    .unwrap_or_else(|| "PostHog event".to_string());
    let source_url = extract_string(&payload, &["/data/event/url", "/data/url", "/url"]);

    let project = load_project(&deployment, integrations.project_id).await?;
    let prd_template = effective_prd_template(&project);
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(
        &format!("Title: {}\n\nPayload:\n{}", title, raw_payload_json),
        prd_template,
    )
    .await;
    let (kind, status, prd_markdown) = match classification {
        Some(result) if result.actionable && !matches!(result.kind, InboxItemKind::Other) => {
            (result.kind, InboxItemStatus::Pending, Some(result.prd_markdown))
        }
        Some(result) => (result.kind, InboxItemStatus::Ignored, Some(result.prd_markdown)),
        None => (InboxItemKind::Other, InboxItemStatus::Pending, None),
    };

    let item = upsert_inbox_item(
        &deployment,
        UpsertInboxItem {
            project_id: integrations.project_id,
            source: InboxSource::Posthog,
            source_item_id,
            source_url,
            title,
            raw_payload_json: Some(raw_payload_json),
            kind,
            status,
            prd_markdown,
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    post_registered_if_needed(&deployment.db().pool, &integrations, &item).await;
    // Post to Slack if configured
    post_prd_to_slack_if_configured(&deployment, &integrations, &item).await;

    Ok(Json(ApiResponse::success(())))
}

pub async fn sentry_webhook(
    State(deployment): State<DeploymentImpl>,
    Path(webhook_token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    let integrations = load_project_integrations(&deployment, &webhook_token).await?;
    if let Some(secret) = integrations.sentry_webhook_secret.as_ref() {
        let signature = headers
            .get("Sentry-Hook-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized)?;
        if !verify_hmac_sha256(secret, signature, &body) {
            return Err(ApiError::Unauthorized);
        }
    }

    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let source_item_id = extract_string(
        &payload,
        &[
            "/data/issue/id",
            "/issue/id",
            "/data/event/event_id",
            "/event/event_id",
            "/event_id",
            "/id",
        ],
    )
    .ok_or_else(|| ApiError::BadRequest("Missing Sentry issue id".to_string()))?;
    let title = extract_text(
        &payload,
        &[
            "/data/issue/title",
            "/issue/title",
            "/title",
            "/message",
            "/culprit",
        ],
    )
    .unwrap_or_else(|| "Sentry issue".to_string());
    let source_url = extract_string(
        &payload,
        &["/data/issue/url", "/issue/url", "/url"],
    );

    let project = load_project(&deployment, integrations.project_id).await?;
    let prd_template = effective_prd_template(&project);
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(
        &format!("Title: {}\n\nPayload:\n{}", title, raw_payload_json),
        prd_template,
    )
    .await;
    let (kind, status, prd_markdown) = match classification {
        Some(result) if result.actionable && !matches!(result.kind, InboxItemKind::Other) => {
            (result.kind, InboxItemStatus::Pending, Some(result.prd_markdown))
        }
        Some(result) => (result.kind, InboxItemStatus::Ignored, Some(result.prd_markdown)),
        None => (InboxItemKind::Other, InboxItemStatus::Pending, None),
    };

    let item = upsert_inbox_item(
        &deployment,
        UpsertInboxItem {
            project_id: integrations.project_id,
            source: InboxSource::Sentry,
            source_item_id,
            source_url,
            title,
            raw_payload_json: Some(raw_payload_json),
            kind,
            status,
            prd_markdown,
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        Uuid::new_v4(),
    )
    .await?;

    post_registered_if_needed(&deployment.db().pool, &integrations, &item).await;
    // Post to Slack if configured
    post_prd_to_slack_if_configured(&deployment, &integrations, &item).await;

    Ok(Json(ApiResponse::success(())))
}

// === Slack Webhook Handlers ===

#[derive(Debug, Deserialize, Serialize)]
struct SlackSlashCommand {
    token: Option<String>,
    team_id: String,
    team_domain: Option<String>,
    channel_id: String,
    channel_name: Option<String>,
    user_id: String,
    user_name: Option<String>,
    command: String,
    text: String,
    response_url: Option<String>,
    trigger_id: String,
}

#[derive(Debug, Deserialize)]
struct SlackInteractivityPayload {
    #[serde(rename = "type")]
    payload_type: String,
    user: SlackUser,
    trigger_id: String,
    #[serde(default)]
    actions: Vec<SlackAction>,
    #[serde(default)]
    view: Option<SlackViewPayload>,
    #[serde(default)]
    channel: Option<SlackChannel>,
    #[serde(default)]
    message: Option<SlackMessage>,
}

#[derive(Debug, Deserialize)]
struct SlackUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct SlackChannel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct SlackMessage {
    ts: String,
}

#[derive(Debug, Deserialize)]
struct SlackAction {
    action_id: String,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackViewPayload {
    #[serde(default)]
    callback_id: Option<String>,
    #[serde(default)]
    private_metadata: Option<String>,
    #[serde(default)]
    state: Option<SlackViewState>,
}

#[derive(Debug, Deserialize)]
struct SlackViewState {
    values: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SlackCommandResponse {
    response_type: String, // "ephemeral" or "in_channel"
    text: String,
}

/// Find project integrations by Slack channel ID
async fn find_integrations_by_slack_channel(
    deployment: &DeploymentImpl,
    channel_id: &str,
) -> Result<ProjectIntegrations, ApiError> {
    // Find all integrations and filter by slack_channel_id
    let all_integrations = ProjectIntegrations::find_all(&deployment.db().pool).await?;
    all_integrations
        .into_iter()
        .find(|i| i.slack_channel_id.as_deref() == Some(channel_id))
        .ok_or_else(|| ApiError::BadRequest("No project configured for this Slack channel".to_string()))
}

/// Handle Slack slash commands (e.g., /vibe)
/// 
/// IMPORTANT: Slack requires a response within 3 seconds. We respond immediately
/// with an acknowledgment and process the command asynchronously.
pub async fn slack_commands(
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SlackCommandResponse>, ApiError> {
    // Parse the form-encoded body
    let body_str = String::from_utf8_lossy(&body);
    let command: SlackSlashCommand = serde_urlencoded::from_str(&body_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid slash command payload: {}", e)))?;

    // Find integrations for this channel
    let integrations = find_integrations_by_slack_channel(&deployment, &command.channel_id).await?;

    // Verify Slack signature
    if let Some(secret) = integrations.slack_signing_secret.as_ref() {
        let timestamp = headers
            .get("X-Slack-Request-Timestamp")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;
        let signature = headers
            .get("X-Slack-Signature")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        slack::verify_slack_signature(secret, timestamp, &body, signature)
            .map_err(|_| ApiError::Unauthorized)?;
    }

    // If no text provided, return usage info
    if command.text.trim().is_empty() {
        return Ok(Json(SlackCommandResponse {
            response_type: "ephemeral".to_string(),
            text: "Usage: /vibe <description of the task or feature request>".to_string(),
        }));
    }

    // Clone what we need for the async task
    let command_text = command.text.clone();
    let channel_id = command.channel_id.clone();
    let user_id = command.user_id.clone();
    let project_id = integrations.project_id;
    let bot_token = integrations.slack_bot_token.clone();

    // Spawn async task to process the command
    // This runs in the background after we respond to Slack
    tokio::spawn(async move {
        process_slack_command_async(
            deployment,
            command,
            command_text,
            channel_id,
            user_id,
            project_id,
            bot_token,
        ).await;
    });

    // Respond immediately to Slack (must be within 3 seconds)
    Ok(Json(SlackCommandResponse {
        response_type: "ephemeral".to_string(),
        text: "Processing your request... A PRD will be posted to this channel shortly.".to_string(),
    }))
}

/// Process Slack command asynchronously after responding to Slack
async fn process_slack_command_async(
    deployment: DeploymentImpl,
    command: SlackSlashCommand,
    command_text: String,
    channel_id: String,
    user_id: String,
    project_id: Uuid,
    bot_token: Option<String>,
) {
    let prd_template = match load_project(&deployment, project_id).await {
        Ok(project) => effective_prd_template(&project).to_string(),
        Err(err) => {
            warn!("Failed to load project for Slack command: {}", err);
            DEFAULT_INBOX_PRD_TEMPLATE.to_string()
        }
    };
    // Generate PRD from the text
    let classification = classify_payload(&command_text, &prd_template).await;
    let (title, kind, prd_markdown): (String, InboxItemKind, String) = match classification {
        Some(result) => (
            if result.title.is_empty() { "Task from Slack".to_string() } else { result.title },
            result.kind,
            result.prd_markdown,
        ),
        None => (
            "Task from Slack".to_string(),
            InboxItemKind::Feature,
            command_text.clone(),
        ),
    };

    // Create inbox item
    let inbox_item_id = Uuid::new_v4();
    let action_token = Uuid::new_v4().to_string();
    
    let item = match InboxItem::create(
        &deployment.db().pool,
        &CreateInboxItem {
            project_id,
            source: InboxSource::Slack,
            source_item_id: inbox_item_id.to_string(),
            source_url: None,
            title: title.clone(),
            raw_payload_json: Some(serde_json::to_string(&command).unwrap_or_default()),
            kind: kind.clone(),
            status: InboxItemStatus::Pending,
            prd_markdown: Some(prd_markdown.clone()),
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        inbox_item_id,
    )
    .await {
        Ok(item) => item,
        Err(e) => {
            warn!("Failed to create inbox item from Slack command: {}", e);
            return;
        }
    };

    // Post PRD to Slack channel with user mention
    if let Some(bot_token) = bot_token.as_ref() {
        let client = SlackClient::new(bot_token);
        let blocks = build_prd_blocks_json(
            &title,
            &format!("{:?}", kind).to_lowercase(),
            "slack",
            &prd_markdown,
            &item.id.to_string(),
            PrdMessageStatus::Pending,
        );

        // Include user mention in the message text (shows in notifications and as fallback)
        let message_text = format!("<@{}> created: {}", user_id, title);

        match client.post_message_json(&channel_id, &message_text, blocks, None).await {
            Ok(result) => {
                // Store the Slack message reference
                if let Err(e) = InboxItem::set_slack_message(
                    &deployment.db().pool,
                    item.id,
                    &result.channel,
                    &result.ts,
                ).await {
                    warn!("Failed to store Slack message reference: {}", e);
                }
                info!("Posted PRD to Slack channel {} for user {}", channel_id, user_id);
            }
            Err(e) => {
                warn!("Failed to post PRD to Slack: {}", e);
            }
        }
    }
}

/// Handle Slack interactivity (buttons, modals)
pub async fn slack_interactivity(
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<String, ApiError> {
    // Parse the form-encoded payload
    let body_str = String::from_utf8_lossy(&body);
    let params: std::collections::HashMap<String, String> = serde_urlencoded::from_str(&body_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid interactivity payload: {}", e)))?;

    let payload_str = params.get("payload")
        .ok_or_else(|| ApiError::BadRequest("Missing payload".to_string()))?;
    
    let payload: SlackInteractivityPayload = serde_json::from_str(payload_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid payload JSON: {}", e)))?;

    info!("Slack interactivity: type={}, user={}", payload.payload_type, payload.user.id);

    match payload.payload_type.as_str() {
        "block_actions" => {
            handle_block_actions(&deployment, &headers, &body, &payload).await
        }
        "view_submission" => {
            handle_view_submission(&deployment, &headers, &body, &payload).await
        }
        _ => {
            warn!("Unhandled Slack interactivity type: {}", payload.payload_type);
            Ok(String::new())
        }
    }
}

async fn handle_block_actions(
    deployment: &DeploymentImpl,
    headers: &HeaderMap,
    body: &[u8],
    payload: &SlackInteractivityPayload,
) -> Result<String, ApiError> {
    let action = payload.actions.first()
        .ok_or_else(|| ApiError::BadRequest("No action in payload".to_string()))?;
    
    let inbox_item_id = action.value.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Missing inbox item ID in action value".to_string()))?;
    
    let inbox_item_id = Uuid::parse_str(inbox_item_id)
        .map_err(|_| ApiError::BadRequest("Invalid inbox item ID".to_string()))?;
    
    let mut item = InboxItem::find_by_id(&deployment.db().pool, inbox_item_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;
    
    let integrations = ProjectIntegrations::find_by_project_id(&deployment.db().pool, item.project_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project integrations not found".to_string()))?;

    // Verify signature
    if let Some(secret) = integrations.slack_signing_secret.as_ref() {
        let timestamp = headers
            .get("X-Slack-Request-Timestamp")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;
        let signature = headers
            .get("X-Slack-Signature")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        slack::verify_slack_signature(secret, timestamp, body, signature)
            .map_err(|_| ApiError::Unauthorized)?;
    }

    // Extract channel_id and message_ts from payload and store if missing on item
    // This ensures we have the Slack reference for future updates
    if item.slack_channel_id.is_none() || item.slack_message_ts.is_none() {
        if let (Some(channel), Some(message)) = (&payload.channel, &payload.message) {
            info!("Storing Slack message reference: channel={}, ts={}", channel.id, message.ts);
            if let Err(e) = InboxItem::set_slack_message(
                &deployment.db().pool,
                item.id,
                &channel.id,
                &message.ts,
            ).await {
                warn!("Failed to store Slack message reference: {}", e);
            } else {
                // Update our local copy with the new values
                item.slack_channel_id = Some(channel.id.clone());
                item.slack_message_ts = Some(message.ts.clone());
            }
        }
    }

    let bot_token = integrations.slack_bot_token.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Slack bot token not configured".to_string()))?;
    let client = SlackClient::new(bot_token);

    match action.action_id.as_str() {
        "prd_accept" => {
            // Fetch project repos and collect available branches
            let project_repos = db::models::project_repo::ProjectRepo::find_repos_for_project(
                &deployment.db().pool, 
                item.project_id
            ).await.unwrap_or_default();
            
            info!("Slack accept: found {} repos for project {}", project_repos.len(), item.project_id);
            
            // Collect unique branches from all repos
            let mut all_branches: Vec<String> = Vec::new();
            for repo in &project_repos {
                match deployment.git().get_all_branches(&repo.path) {
                    Ok(branches) => {
                        info!("Repo {} has {} branches", repo.name, branches.len());
                        for branch in branches {
                            if !all_branches.contains(&branch.name) {
                                all_branches.push(branch.name);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get branches for repo {}: {}", repo.name, e);
                    }
                }
            }
            
            // Sort branches with main/master first
            all_branches.sort_by(|a, b| {
                let a_priority = if a == "main" { 0 } else if a == "master" { 1 } else { 2 };
                let b_priority = if b == "main" { 0 } else if b == "master" { 1 } else { 2 };
                a_priority.cmp(&b_priority).then(a.cmp(b))
            });
            
            info!("Slack accept: collected {} unique branches", all_branches.len());
            
            // Open the accept modal with branch options
            let view_json = build_accept_modal_json(&item.id.to_string(), &all_branches);
            client.views_open_json(&payload.trigger_id, view_json).await
                .map_err(|e| ApiError::BadRequest(format!("Failed to open modal: {}", e)))?;
        }
        "prd_update" => {
            // Open the update modal using JSON for proper input blocks
            let prd = item.prd_markdown.as_deref().unwrap_or("");
            let view_json = build_update_modal_view_json(&item.id.to_string(), &item.title, prd);
            client.views_open_json(&payload.trigger_id, view_json).await
                .map_err(|e| ApiError::BadRequest(format!("Failed to open modal: {}", e)))?;
        }
        "prd_decline" => {
            // Update status to declined
            InboxItem::update(
                &deployment.db().pool,
                item.id,
                &db::models::inbox_item::UpdateInboxItem {
                    title: None,
                    kind: None,
                    status: Some(InboxItemStatus::Declined),
                    prd_markdown: None,
                    task_id: None,
                    linear_issue_id: None,
                    linear_issue_url: None,
                    outbound_last_error: None,
                },
            ).await?;

            // Delete the Slack message
            if let (Some(channel_id), Some(ts)) = (item.slack_channel_id.as_ref(), item.slack_message_ts.as_ref()) {
                let _ = client.delete_message(channel_id, ts).await;
            }
        }
        _ => {
            warn!("Unknown action: {}", action.action_id);
        }
    }

    Ok(String::new())
}

async fn handle_view_submission(
    deployment: &DeploymentImpl,
    headers: &HeaderMap,
    body: &[u8],
    payload: &SlackInteractivityPayload,
) -> Result<String, ApiError> {
    let view = payload.view.as_ref()
        .ok_or_else(|| ApiError::BadRequest("No view in payload".to_string()))?;
    
    let callback_id = view.callback_id.as_deref().unwrap_or("");
    
    match callback_id {
        "accept_prd_modal" => {
            handle_accept_modal_submission(deployment, headers, body, payload).await
        }
        "update_prd_modal" => {
            handle_update_modal_submission(deployment, headers, body, payload).await
        }
        _ => {
            warn!("Unknown modal callback: {}", callback_id);
            Ok(String::new())
        }
    }
}

async fn handle_accept_modal_submission(
    deployment: &DeploymentImpl,
    _headers: &HeaderMap,
    _body: &[u8],
    payload: &SlackInteractivityPayload,
) -> Result<String, ApiError> {
    use db::models::project_repo::ProjectRepo;
    use db::models::task::{CreateTask, Task};
    use db::models::workspace::{CreateWorkspace, Workspace};
    use db::models::workspace_repo::CreateWorkspaceRepo;
    use executors::executors::BaseCodingAgent;
    use executors::profile::ExecutorProfileId;
    use services::services::inbox_integrations::linear_create_issue;
    use std::str::FromStr;

    let pool = &deployment.db().pool;
    let view = payload.view.as_ref()
        .ok_or_else(|| ApiError::BadRequest("No view in payload".to_string()))?;
    
    let inbox_item_id = view.private_metadata.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Missing inbox item ID".to_string()))?;
    
    let inbox_item_id = Uuid::parse_str(inbox_item_id)
        .map_err(|_| ApiError::BadRequest("Invalid inbox item ID".to_string()))?;
    
    let item = InboxItem::find_by_id(pool, inbox_item_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

    // Check if already accepted
    if matches!(item.status, InboxItemStatus::Accepted) {
        return Ok(String::new());
    }

    // Parse modal state for model, configuration, and branch selections
    let state = view.state.as_ref();
    let selected_model = state
        .and_then(|s| s.values.pointer("/model_block/model_select/selected_option/value"))
        .and_then(|v| v.as_str())
        .unwrap_or("CLAUDE_CODE");
    let selected_config = state
        .and_then(|s| s.values.pointer("/config_block/config_select/selected_option/value"))
        .and_then(|v| v.as_str())
        .filter(|s| *s != "DEFAULT");  // Treat DEFAULT as None (use executor's default)
    // Try dropdown first (branch_select), then text input (branch_input), then default to "main"
    let selected_branch = state
        .and_then(|s| s.values.pointer("/branch_block/branch_select/selected_option/value"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            state
                .and_then(|s| s.values.pointer("/branch_block/branch_input/value"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("main");
    
    info!("Slack accept modal: model={}, config={:?}, branch={}", selected_model, selected_config, selected_branch);

    let integrations = ProjectIntegrations::find_by_project_id(pool, item.project_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project integrations not found".to_string()))?;

    // Get project repos for creating the attempt
    let project_repos = ProjectRepo::find_repos_for_project(pool, item.project_id).await?;

    // 1. Create task from PRD first (so we have task_id for Linear issue)
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

    info!("Created task {} from inbox item {}", task.id, item.id);

    // 2. Create Linear issue if Linear is configured and source is not Linear
    let mut linear_issue_id = item.linear_issue_id.clone();
    let mut linear_issue_url = item.linear_issue_url.clone();

    if !matches!(item.source, InboxSource::Linear) && linear_issue_id.is_none() {
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
            ).await {
                Ok(issue) => {
                    linear_issue_id = Some(issue.id);
                    linear_issue_url = issue.url;
                    info!("Created Linear issue for inbox item {}", item.id);
                }
                Err(e) => {
                    warn!("Failed to create Linear issue: {}", e);
                    // Continue anyway - Linear is optional
                }
            }
        }
    }

    // 3. Update inbox item with task_id and linear info
    InboxItem::update(
        pool,
        item.id,
        &db::models::inbox_item::UpdateInboxItem {
            title: None,
            kind: None,
            status: Some(InboxItemStatus::Accepted),
            prd_markdown: None,
            task_id: Some(task.id),
            linear_issue_id: linear_issue_id.clone(),
            linear_issue_url: linear_issue_url.clone(),
            outbound_last_error: None,
        },
    ).await?;

    // Store the Slack user who accepted the PRD
    let accepted_by_user_id = payload.user.id.clone();
    if let Err(e) = InboxItem::set_slack_accepted_by(pool, item.id, &accepted_by_user_id).await {
        warn!("Failed to store Slack accepted by user: {}", e);
    }

    // 4. Create workspace/attempt if we have repos
    if !project_repos.is_empty() {
        // Parse executor profile with optional variant/configuration
        let executor = BaseCodingAgent::from_str(selected_model).unwrap_or(BaseCodingAgent::ClaudeCode);
        let executor_profile_id = match selected_config {
            Some(variant) => ExecutorProfileId::with_variant(executor, variant.to_string()),
            None => ExecutorProfileId::new(executor),
        };

        // Compute agent_working_dir based on repo count
        let agent_working_dir = if project_repos.len() == 1 {
            Some(project_repos[0].name.clone())
        } else {
            None
        };

        let attempt_id = Uuid::new_v4();
        let git_branch_name = deployment
            .container()
            .git_branch_from_workspace(&attempt_id, &task.title)
            .await;

        // Create workspace
        let workspace = Workspace::create(
            pool,
            &CreateWorkspace {
                branch: git_branch_name,
                agent_working_dir,
            },
            attempt_id,
            task.id,
        )
        .await?;

        // Add repos to workspace with the selected base branch
        let workspace_repos: Vec<CreateWorkspaceRepo> = project_repos
            .iter()
            .map(|repo| CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: selected_branch.to_string(),
            })
            .collect();
        let _ = db::models::workspace_repo::WorkspaceRepo::create_many(
            pool,
            workspace.id,
            &workspace_repos,
        ).await;

        // Start the workspace
        if let Err(err) = deployment
            .container()
            .start_workspace(&workspace, executor_profile_id.clone())
            .await
        {
            warn!("Failed to start workspace from Slack: {}", err);
        } else {
            info!("Started workspace {} for task {} via Slack", workspace.id, task.id);
        }

        // Post started notification
        services::services::inbox_outbound::post_started_if_needed(pool, &integrations, &item).await;
    }

    // 5. Post Slack notification and update original message
    if let Some(bot_token) = integrations.slack_bot_token.as_ref() {
        let client = SlackClient::new(bot_token);
        
        if let (Some(channel_id), Some(ts)) = (item.slack_channel_id.as_ref(), item.slack_message_ts.as_ref()) {
            // Build VK task URL (absolute URL)
            let vk_task_url = get_vk_task_url(&item.project_id, &task.id);
            
            // Post channel message tagging the user who accepted (with Linear link)
            let channel_blocks = build_task_accepted_message_json(
                &accepted_by_user_id,
                &item.title,
                &vk_task_url,
                linear_issue_url.as_deref(),
            );
            let channel_text = get_task_accepted_text(&accepted_by_user_id, &item.title);
            if let Err(e) = client.post_message_json(channel_id, &channel_text, channel_blocks, None).await {
                warn!("Failed to post Slack channel notification: {}", e);
            }

            // Update original message to show accepted status
            let blocks = build_prd_blocks_json(
                &item.title,
                &format!("{:?}", item.kind).to_lowercase(),
                &format!("{:?}", item.source).to_lowercase(),
                item.prd_markdown.as_deref().unwrap_or(""),
                &item.id.to_string(),
                PrdMessageStatus::Accepted,
            );
            if let Err(e) = client.update_message_json(channel_id, ts, &format!("[Accepted] {}", item.title), blocks).await {
                warn!("Failed to update Slack message: {}", e);
            }
        }
    }

    Ok(String::new())
}

async fn handle_update_modal_submission(
    deployment: &DeploymentImpl,
    _headers: &HeaderMap,
    _body: &[u8],
    payload: &SlackInteractivityPayload,
) -> Result<String, ApiError> {
    let view = payload.view.as_ref()
        .ok_or_else(|| ApiError::BadRequest("No view in payload".to_string()))?;
    
    // Parse private_metadata which contains the inbox_item_id
    let metadata: serde_json::Value = view.private_metadata.as_ref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_default();
    
    let inbox_item_id = metadata.get("inbox_item_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("Missing inbox item ID".to_string()))?;
    
    let inbox_item_id = Uuid::parse_str(inbox_item_id)
        .map_err(|_| ApiError::BadRequest("Invalid inbox item ID".to_string()))?;
    
    // Extract values from view state
    let state = view.state.as_ref()
        .ok_or_else(|| ApiError::BadRequest("No state in view".to_string()))?;
    
    let new_title = state.values
        .pointer("/title_block/title_input/value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let new_prd = state.values
        .pointer("/prd_block/prd_input/value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let item = InboxItem::find_by_id(&deployment.db().pool, inbox_item_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Inbox item not found".to_string()))?;

    // Update the inbox item
    let updated_item = InboxItem::update(
        &deployment.db().pool,
        item.id,
        &db::models::inbox_item::UpdateInboxItem {
            title: new_title.clone(),
            kind: None,
            status: None,
            prd_markdown: new_prd.clone(),
            task_id: None,
            linear_issue_id: None,
            linear_issue_url: None,
            outbound_last_error: None,
        },
    ).await?;

    // Update the Slack message
    let integrations = ProjectIntegrations::find_by_project_id(&deployment.db().pool, item.project_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project integrations not found".to_string()))?;

    if let Some(bot_token) = integrations.slack_bot_token.as_ref() {
        let client = SlackClient::new(bot_token);
        
        if let (Some(channel_id), Some(ts)) = (item.slack_channel_id.as_ref(), item.slack_message_ts.as_ref()) {
            info!("Updating Slack message: channel={}, ts={}", channel_id, ts);
            let blocks = build_prd_blocks_json(
                &updated_item.title,
                &format!("{:?}", updated_item.kind).to_lowercase(),
                &format!("{:?}", updated_item.source).to_lowercase(),
                updated_item.prd_markdown.as_deref().unwrap_or(""),
                &updated_item.id.to_string(),
                PrdMessageStatus::Pending,
            );
            match client.update_message_json(channel_id, ts, &updated_item.title, blocks).await {
                Ok(_) => info!("Successfully updated Slack message"),
                Err(e) => warn!("Failed to update Slack message: {}", e),
            }
        } else {
            warn!("Cannot update Slack message: channel_id={:?}, ts={:?}", item.slack_channel_id, item.slack_message_ts);
        }
    } else {
        warn!("Cannot update Slack message: no bot token configured");
    }

    Ok(String::new())
}

/// Build update modal with proper input blocks using raw JSON
fn build_update_modal_view_json(inbox_item_id: &str, current_title: &str, current_prd: &str) -> serde_json::Value {
    // Truncate PRD if too long (Slack has a 3000 char limit for text inputs)
    let truncated_prd = if current_prd.len() > 2800 {
        &current_prd[..2800]
    } else {
        current_prd
    };
    
    serde_json::json!({
        "type": "modal",
        "callback_id": "update_prd_modal",
        "private_metadata": serde_json::json!({
            "inbox_item_id": inbox_item_id
        }).to_string(),
        "title": {
            "type": "plain_text",
            "text": "Update PRD",
            "emoji": true
        },
        "submit": {
            "type": "plain_text",
            "text": "Save",
            "emoji": true
        },
        "close": {
            "type": "plain_text",
            "text": "Cancel",
            "emoji": true
        },
        "blocks": [
            {
                "type": "input",
                "block_id": "title_block",
                "label": {
                    "type": "plain_text",
                    "text": "Title",
                    "emoji": true
                },
                "element": {
                    "type": "plain_text_input",
                    "action_id": "title_input",
                    "initial_value": current_title
                }
            },
            {
                "type": "input",
                "block_id": "prd_block",
                "label": {
                    "type": "plain_text",
                    "text": "PRD Content",
                    "emoji": true
                },
                "element": {
                    "type": "plain_text_input",
                    "action_id": "prd_input",
                    "multiline": true,
                    "initial_value": truncated_prd
                }
            }
        ]
    })
}

// === Personal AI Quick Endpoint ===

/// Handle the personal-ai quick endpoint.
/// Creates a PRD, posts to Slack, auto-accepts, and starts Claude Code immediately.
pub async fn personal_ai_quick(
    State(deployment): State<DeploymentImpl>,
    Path(webhook_token): Path<String>,
    Json(payload): Json<PersonalAiPayload>,
) -> Result<Json<ApiResponse<PersonalAiResponse>>, ApiError> {
    use db::models::inbox_item::UpdateInboxItem;
    use db::models::project_repo::ProjectRepo;
    use db::models::task::{CreateTask, Task};
    use db::models::workspace::{CreateWorkspace, Workspace};
    use db::models::workspace_repo::CreateWorkspaceRepo;
    use executors::executors::BaseCodingAgent;
    use executors::profile::ExecutorProfileId;
    use services::services::inbox_integrations::linear_create_issue;

    let pool = &deployment.db().pool;

    // 1. Load project integrations using webhook token
    let integrations = load_project_integrations(&deployment, &webhook_token).await?;
    let project = load_project(&deployment, integrations.project_id).await?;
    let prd_template = effective_prd_template(&project);

    // 2. Generate PRD using Anthropic (with fallback to raw text)
    let classification = classify_payload(&payload.text, prd_template).await;
    let (title, kind, prd_markdown): (String, InboxItemKind, String) = match classification {
        Some(result) => (
            payload.title.clone().unwrap_or_else(|| {
                if result.title.is_empty() {
                    "Quick task from Personal AI".to_string()
                } else {
                    result.title
                }
            }),
            result.kind,
            result.prd_markdown,
        ),
        None => (
            payload.title.clone().unwrap_or_else(|| "Quick task from Personal AI".to_string()),
            InboxItemKind::Feature,
            payload.text.clone(),
        ),
    };

    // 3. Create inbox item
    let inbox_item_id = Uuid::new_v4();
    let action_token = Uuid::new_v4().to_string();
    let source_item_id = payload.source_item_id.clone().unwrap_or_else(|| inbox_item_id.to_string());
    let raw_payload_json = serde_json::json!({
        "text": payload.text,
        "title": payload.title,
        "source_url": payload.source_url,
        "base_branch": payload.base_branch,
        "slack_user_id": payload.slack_user_id,
    }).to_string();

    let item = InboxItem::create(
        pool,
        &CreateInboxItem {
            project_id: integrations.project_id,
            source: InboxSource::Manual,
            source_item_id,
            source_url: payload.source_url.clone(),
            title: title.clone(),
            raw_payload_json: Some(raw_payload_json),
            kind: kind.clone(),
            status: InboxItemStatus::Pending, // Will be updated to Accepted after task creation
            prd_markdown: Some(prd_markdown.clone()),
            action_token,
            linear_issue_id: None,
            linear_issue_url: None,
        },
        inbox_item_id,
    )
    .await?;

    info!("Created inbox item {} for personal-ai quick request", item.id);

    // 4. Post to Slack (best-effort) - show as Accepted immediately (no buttons)
    let mut slack_posted = false;
    let mut slack_channel_id: Option<String> = None;
    let mut slack_message_ts: Option<String> = None;

    if let (Some(bot_token), Some(channel_id)) = (
        integrations.slack_bot_token.as_ref(),
        integrations.slack_channel_id.as_ref(),
    ) {
        let client = SlackClient::new(bot_token);
        let kind_str = format!("{:?}", kind).to_lowercase();

        // Build blocks with Accepted status (no buttons)
        let blocks = build_prd_blocks_json(
            &title,
            &kind_str,
            "personal-ai",
            &prd_markdown,
            &item.id.to_string(),
            PrdMessageStatus::Accepted,
        );

        // Include user mention if provided
        let message_text = if let Some(ref user_id) = payload.slack_user_id {
            format!("<@{}> created (auto-accepted): {}", user_id, title)
        } else {
            format!("[Auto-accepted] {}", title)
        };

        match client.post_message_json(channel_id, &message_text, blocks, None).await {
            Ok(result) => {
                slack_posted = true;
                slack_channel_id = Some(result.channel.clone());
                slack_message_ts = Some(result.ts.clone());

                // Store Slack message reference
                if let Err(e) = InboxItem::set_slack_message(pool, item.id, &result.channel, &result.ts).await {
                    warn!("Failed to store Slack message reference: {}", e);
                }
                info!("Posted PRD to Slack for personal-ai quick: {} (ts: {})", title, result.ts);
            }
            Err(e) => {
                warn!("Failed to post PRD to Slack (best-effort): {}", e);
            }
        }
    }

    // Store Slack user ID if provided
    if let Some(ref user_id) = payload.slack_user_id {
        if let Err(e) = InboxItem::set_slack_accepted_by(pool, item.id, user_id).await {
            warn!("Failed to store Slack accepted by user: {}", e);
        }
    }

    // 5. Create task from PRD
    let task = Task::create(
        pool,
        &CreateTask::from_title_description(
            integrations.project_id,
            title.clone(),
            Some(prd_markdown.clone()),
        ),
        Uuid::new_v4(),
    )
    .await?;

    info!("Created task {} from personal-ai quick request", task.id);

    // 6. Create Linear issue if configured (best-effort)
    let mut linear_issue_id: Option<String> = None;
    let mut linear_issue_url: Option<String> = None;

    if let (Some(api_key), Some(team_id)) = (
        integrations.linear_api_key.as_deref(),
        integrations.linear_team_id.as_deref(),
    ) {
        let mut description = prd_markdown.clone();

        // Add VK task link at the top of description
        let vk_task_url = get_vk_task_url(&integrations.project_id, &task.id);
        if !vk_task_url.is_empty() {
            let vk_link = format!("**[View in Vibe Kanban]({})**\n\n", vk_task_url);
            description = format!("{}{}", vk_link, description);
        }

        if let Some(ref source_url) = payload.source_url {
            if !description.trim().is_empty() {
                description.push_str("\n\n");
            }
            description.push_str(&format!("Source: {}", source_url));
        }
        if description.trim().is_empty() {
            description = title.clone();
        }

        match linear_create_issue(
            api_key,
            team_id,
            &title,
            &description,
            integrations.linear_state_id_todo.as_deref(),
        ).await {
            Ok(issue) => {
                linear_issue_id = Some(issue.id);
                linear_issue_url = issue.url;
                info!("Created Linear issue for personal-ai quick task {}", task.id);
            }
            Err(e) => {
                warn!("Failed to create Linear issue (best-effort): {}", e);
            }
        }
    }

    // 7. Update inbox item to Accepted status with task_id and linear info
    InboxItem::update(
        pool,
        item.id,
        &UpdateInboxItem {
            title: None,
            kind: None,
            status: Some(InboxItemStatus::Accepted),
            prd_markdown: None,
            task_id: Some(task.id),
            linear_issue_id: linear_issue_id.clone(),
            linear_issue_url: linear_issue_url.clone(),
            outbound_last_error: None,
        },
    ).await?;

    // 8. Create workspace and start Claude Code DEFAULT
    let project_repos = ProjectRepo::find_repos_for_project(pool, integrations.project_id).await?;
    let base_branch = payload.base_branch.as_deref().unwrap_or("main");

    let mut workspace_id: Option<Uuid> = None;
    let mut execution_process_id: Option<Uuid> = None;
    let mut started = false;
    let mut start_error: Option<String> = None;

    if !project_repos.is_empty() {
        // Claude Code with DEFAULT profile (no approvals)
        let executor_profile_id = ExecutorProfileId::new(BaseCodingAgent::ClaudeCode);

        // Compute agent_working_dir based on repo count
        let agent_working_dir = if project_repos.len() == 1 {
            Some(project_repos[0].name.clone())
        } else {
            None
        };

        let attempt_id = Uuid::new_v4();
        let git_branch_name = deployment
            .container()
            .git_branch_from_workspace(&attempt_id, &task.title)
            .await;

        // Create workspace
        let workspace = Workspace::create(
            pool,
            &CreateWorkspace {
                branch: git_branch_name,
                agent_working_dir,
            },
            attempt_id,
            task.id,
        )
        .await?;

        workspace_id = Some(workspace.id);

        // Add repos to workspace with the specified base branch
        let workspace_repos: Vec<CreateWorkspaceRepo> = project_repos
            .iter()
            .map(|repo| CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: base_branch.to_string(),
            })
            .collect();
        let _ = db::models::workspace_repo::WorkspaceRepo::create_many(
            pool,
            workspace.id,
            &workspace_repos,
        ).await;

        // Start the workspace
        match deployment
            .container()
            .start_workspace(&workspace, executor_profile_id)
            .await
        {
            Ok(exec_process) => {
                execution_process_id = Some(exec_process.id);
                started = true;
                info!(
                    "Started workspace {} for task {} via personal-ai quick",
                    workspace.id, task.id
                );
            }
            Err(err) => {
                start_error = Some(err.to_string());
                warn!("Failed to start workspace from personal-ai quick: {}", err);
            }
        }

        // Post started notification (best-effort)
        services::services::inbox_outbound::post_started_if_needed(pool, &integrations, &item).await;
    } else {
        start_error = Some("No repositories configured for project".to_string());
    }

    Ok(Json(ApiResponse::success(PersonalAiResponse {
        inbox_item_id: item.id,
        task_id: task.id,
        task_url: get_vk_task_url(&integrations.project_id, &task.id),
        workspace_id,
        execution_process_id,
        slack_posted,
        slack_channel_id,
        slack_message_ts,
        started,
        start_error,
    })))
}

pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/linear/{webhook_token}", post(linear_webhook))
        .route("/intercom/{webhook_token}", post(intercom_webhook))
        .route("/modjo/{webhook_token}", post(modjo_webhook))
        .route("/manual/{webhook_token}", post(manual_webhook))
        .route("/posthog/{webhook_token}", post(posthog_webhook))
        .route("/sentry/{webhook_token}", post(sentry_webhook))
        .route("/slack/commands", post(slack_commands))
        .route("/slack/interactivity", post(slack_interactivity))
        .route("/personal-ai/{webhook_token}", post(personal_ai_quick))
}
