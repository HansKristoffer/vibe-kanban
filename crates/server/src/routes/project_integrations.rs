use std::env;

use axum::{
    Extension, Json, Router,
    extract::Query,
    routing::get,
    extract::State,
};
use chrono::Utc;
use db::models::project::Project;
use db::models::project_integrations::{ProjectIntegrations, UpsertProjectIntegrations};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;
use ts_rs::TS;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};
use deployment::Deployment;
use utils::response::ApiResponse;

#[derive(Debug, Serialize, TS)]
pub struct WebhookUrls {
    pub linear: String,
    pub intercom: String,
    pub modjo: String,
    pub manual: String,
    pub personal_ai: String,
    pub posthog: String,
    pub sentry: String,
    pub slack_commands: String,
    pub slack_interactivity: String,
}

#[derive(Debug, Serialize, TS)]
pub struct ProjectIntegrationsResponse {
    pub webhook_token: String,
    pub webhook_urls: Option<WebhookUrls>,
    pub linear_api_key: Option<String>,
    pub linear_team_id: Option<String>,
    pub linear_state_id_todo: Option<String>,
    pub linear_state_id_inprogress: Option<String>,
    pub linear_state_id_inreview: Option<String>,
    pub linear_state_id_done: Option<String>,
    pub linear_state_id_cancelled: Option<String>,
    pub linear_webhook_secret: Option<String>,
    pub intercom_access_token: Option<String>,
    pub intercom_webhook_secret: Option<String>,
    pub intercom_admin_id: Option<String>,
    pub modjo_api_key: Option<String>,
    pub modjo_webhook_secret: Option<String>,
    pub posthog_webhook_secret: Option<String>,
    pub sentry_webhook_secret: Option<String>,
    pub posthog_api_key: Option<String>,
    pub posthog_host: Option<String>,
    pub posthog_project_id: Option<String>,
    pub sentry_api_token: Option<String>,
    pub sentry_org_slug: Option<String>,
    pub sentry_project_slug: Option<String>,
    pub slack_bot_token: Option<String>,
    pub slack_signing_secret: Option<String>,
    pub slack_channel_id: Option<String>,
    pub linear_api_key_configured: bool,
    pub linear_webhook_secret_configured: bool,
    pub intercom_access_token_configured: bool,
    pub intercom_webhook_secret_configured: bool,
    pub modjo_api_key_configured: bool,
    pub modjo_webhook_secret_configured: bool,
    pub posthog_webhook_secret_configured: bool,
    pub sentry_webhook_secret_configured: bool,
    pub posthog_api_key_configured: bool,
    pub sentry_api_token_configured: bool,
    pub slack_bot_token_configured: bool,
    pub slack_signing_secret_configured: bool,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct LinearTeam {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct LinearWorkflowState {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub state_type: String,
}

#[derive(Debug, Deserialize, TS)]
pub struct UpdateProjectIntegrationsRequest {
    pub linear_api_key: Option<String>,
    pub linear_team_id: Option<String>,
    pub linear_state_id_todo: Option<String>,
    pub linear_state_id_inprogress: Option<String>,
    pub linear_state_id_inreview: Option<String>,
    pub linear_state_id_done: Option<String>,
    pub linear_state_id_cancelled: Option<String>,
    pub linear_webhook_secret: Option<String>,
    pub intercom_access_token: Option<String>,
    pub intercom_webhook_secret: Option<String>,
    pub intercom_admin_id: Option<String>,
    pub modjo_api_key: Option<String>,
    pub modjo_webhook_secret: Option<String>,
    pub posthog_webhook_secret: Option<String>,
    pub sentry_webhook_secret: Option<String>,
    pub posthog_api_key: Option<String>,
    pub posthog_host: Option<String>,
    pub posthog_project_id: Option<String>,
    pub sentry_api_token: Option<String>,
    pub sentry_org_slug: Option<String>,
    pub sentry_project_slug: Option<String>,
    pub slack_bot_token: Option<String>,
    pub slack_signing_secret: Option<String>,
    pub slack_channel_id: Option<String>,
    pub clear_linear_api_key: Option<bool>,
    pub clear_linear_webhook_secret: Option<bool>,
    pub clear_intercom_access_token: Option<bool>,
    pub clear_intercom_webhook_secret: Option<bool>,
    pub clear_modjo_api_key: Option<bool>,
    pub clear_modjo_webhook_secret: Option<bool>,
    pub clear_posthog_webhook_secret: Option<bool>,
    pub clear_sentry_webhook_secret: Option<bool>,
    pub clear_posthog_api_key: Option<bool>,
    pub clear_sentry_api_token: Option<bool>,
    pub clear_slack_bot_token: Option<bool>,
    pub clear_slack_signing_secret: Option<bool>,
}

#[derive(Debug, Deserialize, TS)]
pub struct LinearStatesQuery {
    pub team_id: String,
}

#[derive(Debug, Serialize)]
struct GraphqlRequest<'a> {
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

fn linear_authorization_header_value(raw_token: &str) -> String {
    let trimmed = raw_token.trim();
    let lower = trimmed.to_ascii_lowercase();
    let (provided_bearer, token) = if lower.starts_with("bearer ") {
        (true, trimmed[7..].trim())
    } else {
        (false, trimmed)
    };

    if token.starts_with("lin_api_") {
        token.to_string()
    } else if token.starts_with("lin_oauth_") || provided_bearer {
        format!("Bearer {}", token)
    } else {
        token.to_string()
    }
}

async fn linear_graphql<T: DeserializeOwned>(
    api_key: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, ApiError> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.linear.app/graphql")
        .header("Authorization", linear_authorization_header_value(api_key))
        .json(&GraphqlRequest { query, variables })
        .send()
        .await
        .map_err(|err| ApiError::BadRequest(format!("Linear API request failed: {}", err)))?;

    let status = response.status();
    let body: GraphqlResponse<T> = response
        .json()
        .await
        .map_err(|err| ApiError::BadRequest(format!("Failed to parse Linear response: {}", err)))?;
    if !status.is_success() {
        let message = body
            .errors
            .and_then(|e| e.first().map(|err| err.message.clone()))
            .unwrap_or_else(|| format!("Linear API returned {}", status));
        return Err(ApiError::BadRequest(message));
    }

    if let Some(errors) = body.errors {
        if let Some(error) = errors.first() {
            return Err(ApiError::BadRequest(error.message.clone()));
        }
    }

    body.data.ok_or_else(|| ApiError::BadRequest("Linear API returned no data".to_string()))
}

fn mask_secret(value: &Option<String>) -> Option<String> {
    value.as_ref().map(|_| "********".to_string())
}

fn trim_to_option(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn merge_secret(existing: Option<String>, incoming: Option<String>, clear: bool) -> Option<String> {
    if clear {
        None
    } else if let Some(value) = trim_to_option(incoming) {
        Some(value)
    } else {
        existing
    }
}

fn build_webhook_urls(token: &str) -> Option<WebhookUrls> {
    let base = env::var("VK_PUBLIC_BASE_URL").ok().map(|s| s.trim_end_matches('/').to_string())?;
    Some(WebhookUrls {
        linear: format!("{}/api/webhooks/linear/{}", base, token),
        intercom: format!("{}/api/webhooks/intercom/{}", base, token),
        modjo: format!("{}/api/webhooks/modjo/{}", base, token),
        manual: format!("{}/api/webhooks/manual/{}", base, token),
        personal_ai: format!("{}/api/webhooks/personal-ai/{}", base, token),
        posthog: format!("{}/api/webhooks/posthog/{}", base, token),
        sentry: format!("{}/api/webhooks/sentry/{}", base, token),
        slack_commands: format!("{}/api/webhooks/slack/commands", base),
        slack_interactivity: format!("{}/api/webhooks/slack/interactivity", base),
    })
}

fn to_response(record: ProjectIntegrations) -> ProjectIntegrationsResponse {
    ProjectIntegrationsResponse {
        webhook_token: record.webhook_token.clone(),
        webhook_urls: build_webhook_urls(&record.webhook_token),
        linear_api_key: mask_secret(&record.linear_api_key),
        linear_team_id: record.linear_team_id,
        linear_state_id_todo: record.linear_state_id_todo,
        linear_state_id_inprogress: record.linear_state_id_inprogress,
        linear_state_id_inreview: record.linear_state_id_inreview,
        linear_state_id_done: record.linear_state_id_done,
        linear_state_id_cancelled: record.linear_state_id_cancelled,
        linear_webhook_secret: mask_secret(&record.linear_webhook_secret),
        intercom_access_token: mask_secret(&record.intercom_access_token),
        intercom_webhook_secret: mask_secret(&record.intercom_webhook_secret),
        intercom_admin_id: record.intercom_admin_id,
        modjo_api_key: mask_secret(&record.modjo_api_key),
        modjo_webhook_secret: mask_secret(&record.modjo_webhook_secret),
        posthog_webhook_secret: mask_secret(&record.posthog_webhook_secret),
        sentry_webhook_secret: mask_secret(&record.sentry_webhook_secret),
        posthog_api_key: mask_secret(&record.posthog_api_key),
        posthog_host: record.posthog_host,
        posthog_project_id: record.posthog_project_id,
        sentry_api_token: mask_secret(&record.sentry_api_token),
        sentry_org_slug: record.sentry_org_slug,
        sentry_project_slug: record.sentry_project_slug,
        slack_bot_token: mask_secret(&record.slack_bot_token),
        slack_signing_secret: mask_secret(&record.slack_signing_secret),
        slack_channel_id: record.slack_channel_id,
        linear_api_key_configured: record.linear_api_key.is_some(),
        linear_webhook_secret_configured: record.linear_webhook_secret.is_some(),
        intercom_access_token_configured: record.intercom_access_token.is_some(),
        intercom_webhook_secret_configured: record.intercom_webhook_secret.is_some(),
        modjo_api_key_configured: record.modjo_api_key.is_some(),
        modjo_webhook_secret_configured: record.modjo_webhook_secret.is_some(),
        posthog_webhook_secret_configured: record.posthog_webhook_secret.is_some(),
        sentry_webhook_secret_configured: record.sentry_webhook_secret.is_some(),
        posthog_api_key_configured: record.posthog_api_key.is_some(),
        sentry_api_token_configured: record.sentry_api_token.is_some(),
        slack_bot_token_configured: record.slack_bot_token.is_some(),
        slack_signing_secret_configured: record.slack_signing_secret.is_some(),
    }
}

pub async fn get_project_integrations(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<ApiResponse<ProjectIntegrationsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let existing = ProjectIntegrations::find_by_project_id(pool, project.id).await?;
    let integrations = match existing {
        Some(record) => record,
        None => {
            let token = Uuid::new_v4().to_string();
            ProjectIntegrations::upsert(
                pool,
                project.id,
                &UpsertProjectIntegrations {
                    webhook_token: token,
                    linear_api_key: None,
                    linear_team_id: None,
                    linear_state_id_todo: None,
                    linear_state_id_inprogress: None,
                    linear_state_id_inreview: None,
                    linear_state_id_done: None,
                    linear_state_id_cancelled: None,
                    linear_webhook_secret: None,
                    intercom_access_token: None,
                    intercom_webhook_secret: None,
                    intercom_admin_id: None,
                    modjo_api_key: None,
                    modjo_webhook_secret: None,
                    posthog_webhook_secret: None,
                    sentry_webhook_secret: None,
                    posthog_api_key: None,
                    posthog_host: None,
                    posthog_project_id: None,
                    sentry_api_token: None,
                    sentry_org_slug: None,
                    sentry_project_slug: None,
                    slack_bot_token: None,
                    slack_signing_secret: None,
                    slack_channel_id: None,
                },
            )
            .await?
        }
    };

    Ok(Json(ApiResponse::success(to_response(integrations))))
}

pub async fn update_project_integrations(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<UpdateProjectIntegrationsRequest>,
) -> Result<Json<ApiResponse<ProjectIntegrationsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let existing = ProjectIntegrations::find_by_project_id(pool, project.id)
        .await?
        .unwrap_or_else(|| ProjectIntegrations {
            project_id: project.id,
            webhook_token: Uuid::new_v4().to_string(),
            linear_api_key: None,
            linear_team_id: None,
            linear_state_id_todo: None,
            linear_state_id_inprogress: None,
            linear_state_id_inreview: None,
            linear_state_id_done: None,
            linear_state_id_cancelled: None,
            linear_webhook_secret: None,
            intercom_access_token: None,
            intercom_webhook_secret: None,
            intercom_admin_id: None,
            modjo_api_key: None,
            modjo_webhook_secret: None,
            posthog_webhook_secret: None,
            sentry_webhook_secret: None,
            posthog_api_key: None,
            posthog_host: None,
            posthog_project_id: None,
            sentry_api_token: None,
            sentry_org_slug: None,
            sentry_project_slug: None,
            slack_bot_token: None,
            slack_signing_secret: None,
            slack_channel_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });

    let integrations = ProjectIntegrations::upsert(
        pool,
        project.id,
        &UpsertProjectIntegrations {
            webhook_token: existing.webhook_token,
            linear_api_key: merge_secret(
                existing.linear_api_key,
                payload.linear_api_key,
                payload.clear_linear_api_key.unwrap_or(false),
            ),
            linear_team_id: trim_to_option(payload.linear_team_id).or(existing.linear_team_id),
            linear_state_id_todo: trim_to_option(payload.linear_state_id_todo)
                .or(existing.linear_state_id_todo),
            linear_state_id_inprogress: trim_to_option(payload.linear_state_id_inprogress)
                .or(existing.linear_state_id_inprogress),
            linear_state_id_inreview: trim_to_option(payload.linear_state_id_inreview)
                .or(existing.linear_state_id_inreview),
            linear_state_id_done: trim_to_option(payload.linear_state_id_done)
                .or(existing.linear_state_id_done),
            linear_state_id_cancelled: trim_to_option(payload.linear_state_id_cancelled)
                .or(existing.linear_state_id_cancelled),
            linear_webhook_secret: merge_secret(
                existing.linear_webhook_secret,
                payload.linear_webhook_secret,
                payload.clear_linear_webhook_secret.unwrap_or(false),
            ),
            intercom_access_token: merge_secret(
                existing.intercom_access_token,
                payload.intercom_access_token,
                payload.clear_intercom_access_token.unwrap_or(false),
            ),
            intercom_webhook_secret: merge_secret(
                existing.intercom_webhook_secret,
                payload.intercom_webhook_secret,
                payload.clear_intercom_webhook_secret.unwrap_or(false),
            ),
            intercom_admin_id: trim_to_option(payload.intercom_admin_id)
                .or(existing.intercom_admin_id),
            modjo_api_key: merge_secret(
                existing.modjo_api_key,
                payload.modjo_api_key,
                payload.clear_modjo_api_key.unwrap_or(false),
            ),
            modjo_webhook_secret: merge_secret(
                existing.modjo_webhook_secret,
                payload.modjo_webhook_secret,
                payload.clear_modjo_webhook_secret.unwrap_or(false),
            ),
            posthog_webhook_secret: merge_secret(
                existing.posthog_webhook_secret,
                payload.posthog_webhook_secret,
                payload.clear_posthog_webhook_secret.unwrap_or(false),
            ),
            sentry_webhook_secret: merge_secret(
                existing.sentry_webhook_secret,
                payload.sentry_webhook_secret,
                payload.clear_sentry_webhook_secret.unwrap_or(false),
            ),
            posthog_api_key: merge_secret(
                existing.posthog_api_key,
                payload.posthog_api_key,
                payload.clear_posthog_api_key.unwrap_or(false),
            ),
            posthog_host: trim_to_option(payload.posthog_host).or(existing.posthog_host),
            posthog_project_id: trim_to_option(payload.posthog_project_id)
                .or(existing.posthog_project_id),
            sentry_api_token: merge_secret(
                existing.sentry_api_token,
                payload.sentry_api_token,
                payload.clear_sentry_api_token.unwrap_or(false),
            ),
            sentry_org_slug: trim_to_option(payload.sentry_org_slug).or(existing.sentry_org_slug),
            sentry_project_slug: trim_to_option(payload.sentry_project_slug)
                .or(existing.sentry_project_slug),
            slack_bot_token: merge_secret(
                existing.slack_bot_token,
                payload.slack_bot_token,
                payload.clear_slack_bot_token.unwrap_or(false),
            ),
            slack_signing_secret: merge_secret(
                existing.slack_signing_secret,
                payload.slack_signing_secret,
                payload.clear_slack_signing_secret.unwrap_or(false),
            ),
            slack_channel_id: trim_to_option(payload.slack_channel_id).or(existing.slack_channel_id),
        },
    )
    .await?;

    Ok(Json(ApiResponse::success(to_response(integrations))))
}

pub async fn get_linear_teams(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Json<ApiResponse<Vec<LinearTeam>>>, ApiError> {
    let pool = &deployment.db().pool;
    let integrations = ProjectIntegrations::find_by_project_id(pool, project.id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project integrations not configured".to_string()))?;
    let Some(api_key) = integrations.linear_api_key else {
        return Err(ApiError::BadRequest(
            "Linear API key not configured".to_string(),
        ));
    };

    #[derive(Deserialize)]
    struct TeamsData {
        teams: TeamsConnection,
    }
    #[derive(Deserialize)]
    struct TeamsConnection {
        nodes: Vec<LinearTeam>,
    }

    let data: TeamsData = linear_graphql(
        &api_key,
        "query { teams { nodes { id name } } }",
        None,
    )
    .await?;

    Ok(Json(ApiResponse::success(data.teams.nodes)))
}

pub async fn get_linear_states(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<LinearStatesQuery>,
) -> Result<Json<ApiResponse<Vec<LinearWorkflowState>>>, ApiError> {
    let pool = &deployment.db().pool;
    let integrations = ProjectIntegrations::find_by_project_id(pool, project.id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project integrations not configured".to_string()))?;
    let Some(api_key) = integrations.linear_api_key else {
        return Err(ApiError::BadRequest(
            "Linear API key not configured".to_string(),
        ));
    };

    #[derive(Deserialize)]
    struct TeamData {
        team: TeamStates,
    }
    #[derive(Deserialize)]
    struct TeamStates {
        states: WorkflowStatesConnection,
    }
    #[derive(Deserialize)]
    struct WorkflowStatesConnection {
        nodes: Vec<LinearWorkflowState>,
    }

    let variables = serde_json::json!({ "teamId": query.team_id });
    let data: TeamData = linear_graphql(
        &api_key,
        "query($teamId: String!) { team(id: $teamId) { states { nodes { id name type } } } }",
        Some(variables),
    )
    .await?;

    Ok(Json(ApiResponse::success(data.team.states.nodes)))
}

pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/", get(get_project_integrations).put(update_project_integrations))
        .route("/linear/teams", get(get_linear_teams))
        .route("/linear/states", get(get_linear_states))
}
