use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnrichmentError {
    #[error("Request failed: {0}")]
    Request(String),
    #[error("Unexpected response: {0}")]
    Response(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PosthogEnrichment {
    pub title: String,
    pub description: String,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SentryEnrichment {
    pub title: String,
    pub culprit: Option<String>,
    pub level: Option<String>,
    pub url: Option<String>,
    pub stacktrace: Option<String>,
}

pub async fn fetch_posthog_event(
    api_key: &str,
    host: &str,
    project_id: &str,
    event_id: &str,
) -> Result<PosthogEnrichment, EnrichmentError> {
    let base = host.trim_end_matches('/');
    let url = format!("{}/api/projects/{}/events/{}", base, project_id, event_id);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|err| EnrichmentError::Request(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<empty body>".to_string());
        return Err(EnrichmentError::Response(format!(
            "PostHog API error {}: {}",
            status, body
        )));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| EnrichmentError::Response(err.to_string()))?;

    let title = payload
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("PostHog event")
        .to_string();
    let description = payload
        .get("properties")
        .map(|v| v.to_string())
        .unwrap_or_else(|| payload.to_string());
    let url = payload
        .pointer("/properties/$current_url")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    Ok(PosthogEnrichment {
        title,
        description,
        url,
    })
}

pub async fn fetch_sentry_issue(
    api_token: &str,
    org_slug: &str,
    project_slug: &str,
    issue_id: &str,
) -> Result<SentryEnrichment, EnrichmentError> {
    let client = reqwest::Client::new();
    let issue_url = format!(
        "https://sentry.io/api/0/issues/{}/",
        issue_id
    );
    let issue_response = client
        .get(&issue_url)
        .header("Authorization", format!("Bearer {}", api_token))
        .send()
        .await
        .map_err(|err| EnrichmentError::Request(err.to_string()))?;

    if !issue_response.status().is_success() {
        let status = issue_response.status();
        let body = issue_response
            .text()
            .await
            .unwrap_or_else(|_| "<empty body>".to_string());
        return Err(EnrichmentError::Response(format!(
            "Sentry issue API error {}: {}",
            status, body
        )));
    }

    let issue_payload: serde_json::Value = issue_response
        .json()
        .await
        .map_err(|err| EnrichmentError::Response(err.to_string()))?;

    let title = issue_payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Sentry issue")
        .to_string();
    let culprit = issue_payload
        .get("culprit")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let level = issue_payload
        .get("level")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let permalink = issue_payload
        .get("permalink")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    let latest_event_url = format!(
        "https://sentry.io/api/0/projects/{}/{}/issues/{}/events/latest/",
        org_slug, project_slug, issue_id
    );
    let latest_event_response = client
        .get(&latest_event_url)
        .header("Authorization", format!("Bearer {}", api_token))
        .send()
        .await
        .map_err(|err| EnrichmentError::Request(err.to_string()))?;

    let stacktrace = if latest_event_response.status().is_success() {
        let event_payload: serde_json::Value = latest_event_response
            .json()
            .await
            .unwrap_or(serde_json::Value::Null);
        event_payload
            .pointer("/entries/0/data/values/0/stacktrace/frames")
            .map(|v| v.to_string())
    } else {
        None
    };

    Ok(SentryEnrichment {
        title,
        culprit,
        level,
        url: permalink,
        stacktrace,
    })
}
