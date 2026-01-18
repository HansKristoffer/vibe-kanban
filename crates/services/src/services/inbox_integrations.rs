use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InboxIntegrationError {
    #[error("Request failed: {0}")]
    Request(String),
    #[error("Unexpected response: {0}")]
    Response(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearIssue {
    pub id: String,
    pub url: Option<String>,
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

async fn linear_graphql<T: for<'de> Deserialize<'de>>(
    api_key: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, InboxIntegrationError> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.linear.app/graphql")
        .header("Authorization", linear_authorization_header_value(api_key))
        .json(&GraphqlRequest { query, variables })
        .send()
        .await
        .map_err(|err| InboxIntegrationError::Request(err.to_string()))?;

    let status = response.status();
    let body: GraphqlResponse<T> = response
        .json()
        .await
        .map_err(|err| InboxIntegrationError::Response(err.to_string()))?;

    if !status.is_success() {
        let message = body
            .errors
            .as_ref()
            .and_then(|e| e.first().map(|err| err.message.clone()))
            .unwrap_or_else(|| format!("Linear API returned {}", status));
        return Err(InboxIntegrationError::Response(message));
    }

    if let Some(errors) = body.errors {
        if let Some(error) = errors.first() {
            return Err(InboxIntegrationError::Response(error.message.clone()));
        }
    }

    body.data.ok_or_else(|| InboxIntegrationError::Response("Linear API returned no data".to_string()))
}

pub async fn linear_create_issue(
    api_key: &str,
    team_id: &str,
    title: &str,
    description: &str,
    state_id: Option<&str>,
) -> Result<LinearIssue, InboxIntegrationError> {
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct Data {
        issueCreate: IssueCreate,
    }
    #[derive(Deserialize)]
    struct IssueCreate {
        issue: LinearIssue,
    }

    let variables = serde_json::json!({
        "input": {
            "teamId": team_id,
            "title": title,
            "description": description,
            "stateId": state_id,
        }
    });

    let data: Data = linear_graphql(
        api_key,
        "mutation($input: IssueCreateInput!) { issueCreate(input: $input) { issue { id url } } }",
        Some(variables),
    )
    .await?;

    Ok(data.issueCreate.issue)
}

pub async fn linear_update_issue_state(
    api_key: &str,
    issue_id: &str,
    state_id: &str,
) -> Result<(), InboxIntegrationError> {
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct Data {
        issueUpdate: IssueUpdate,
    }
    #[derive(Deserialize)]
    struct IssueUpdate {
        success: bool,
    }

    let variables = serde_json::json!({
        "id": issue_id,
        "input": { "stateId": state_id }
    });

    let data: Data = linear_graphql(
        api_key,
        "mutation($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success } }",
        Some(variables),
    )
    .await?;

    if data.issueUpdate.success {
        Ok(())
    } else {
        Err(InboxIntegrationError::Response(
            "Linear issue update failed".to_string(),
        ))
    }
}

pub async fn linear_post_comment(
    api_key: &str,
    issue_id: &str,
    body: &str,
) -> Result<(), InboxIntegrationError> {
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct Data {
        issueCommentCreate: IssueCommentCreate,
    }
    #[derive(Deserialize)]
    struct IssueCommentCreate {
        success: bool,
    }

    let variables = serde_json::json!({
        "input": {
            "issueId": issue_id,
            "body": body,
        }
    });

    let data: Data = linear_graphql(
        api_key,
        "mutation($input: IssueCommentCreateInput!) { issueCommentCreate(input: $input) { success } }",
        Some(variables),
    )
    .await?;

    if data.issueCommentCreate.success {
        Ok(())
    } else {
        Err(InboxIntegrationError::Response(
            "Linear comment failed".to_string(),
        ))
    }
}

pub async fn intercom_post_internal_note(
    access_token: &str,
    admin_id: &str,
    conversation_id: &str,
    body: &str,
) -> Result<(), InboxIntegrationError> {
    let client = reqwest::Client::new();
    let response = client
        .post(&format!(
            "https://api.intercom.io/conversations/{}/reply",
            conversation_id
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "message_type": "note",
            "type": "admin",
            "admin_id": admin_id,
            "body": body,
        }))
        .send()
        .await
        .map_err(|err| InboxIntegrationError::Request(err.to_string()))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<empty body>".to_string());
        Err(InboxIntegrationError::Response(format!(
            "Intercom API error {}: {}",
            status, body
        )))
    }
}
