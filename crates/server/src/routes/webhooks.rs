use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::post,
};
use axum::body::Bytes;
use db::models::inbox_item::{InboxItem, InboxItemKind, InboxItemStatus, InboxSource, UpsertInboxItem};
use db::models::project::Project;
use db::models::project_integrations::ProjectIntegrations;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tracing::warn;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};
use deployment::Deployment;
use services::services::anthropic::{AnthropicClient, AnthropicInboxResult};
use services::services::inbox_outbound::post_registered_if_needed;
use services::services::posthog_sentry_enrichment::{
    fetch_posthog_event, fetch_sentry_issue,
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

async fn classify_payload(input: &str) -> Option<AnthropicInboxResult> {
    let client = AnthropicClient::from_env().ok()?;
    match client.classify_and_generate_prd(input).await {
        Ok(result) => Some(result),
        Err(err) => {
            warn!("Anthropic classification failed: {}", err);
            None
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

    let _project = load_project(&deployment, integrations.project_id).await?;
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
    let classification = classify_payload(&classification_input).await;
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

    let _project = load_project(&deployment, integrations.project_id).await?;
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

    let classification = classify_payload(&format!("Title: {}\n\nPayload:\n{}", title, enrichment_text)).await;
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

    let _project = load_project(&deployment, integrations.project_id).await?;
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(&format!("Title: {}\n\nPayload:\n{}", title, raw_payload_json)).await;
    let (kind, status, prd_markdown) = match classification {
        Some(result) if result.actionable && !matches!(result.kind, InboxItemKind::Other) => {
            (result.kind, InboxItemStatus::Pending, Some(result.prd_markdown))
        }
        Some(result) => (result.kind, InboxItemStatus::Ignored, Some(result.prd_markdown)),
        None => (InboxItemKind::Other, InboxItemStatus::Pending, None),
    };

    upsert_inbox_item(
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

    let _project = load_project(&deployment, integrations.project_id).await?;
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(&payload.body).await;
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

    upsert_inbox_item(
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

    let _project = load_project(&deployment, integrations.project_id).await?;
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(&format!("Title: {}\n\nPayload:\n{}", title, raw_payload_json)).await;
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

    let _project = load_project(&deployment, integrations.project_id).await?;
    let action_token = Uuid::new_v4().to_string();
    let raw_payload_json = String::from_utf8_lossy(&body).to_string();
    let classification = classify_payload(&format!("Title: {}\n\nPayload:\n{}", title, raw_payload_json)).await;
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

    Ok(Json(ApiResponse::success(())))
}

pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/linear/{webhook_token}", post(linear_webhook))
        .route("/intercom/{webhook_token}", post(intercom_webhook))
        .route("/modjo/{webhook_token}", post(modjo_webhook))
        .route("/manual/{webhook_token}", post(manual_webhook))
        .route("/posthog/{webhook_token}", post(posthog_webhook))
        .route("/sentry/{webhook_token}", post(sentry_webhook))
}
