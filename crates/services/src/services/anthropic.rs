use std::env;

use db::models::inbox_item::InboxItemKind;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{sleep, Duration};

pub const DEFAULT_INBOX_PRD_TEMPLATE: &str = r#"## Problem Statement
The problem that the user is facing, from the user's perspective.

## Solution
The solution to the problem, from the user's perspective.

## User Stories
A numbered list of user stories in the format:
1. As an <actor>, I want a <feature>, so that <benefit>

Include all relevant user stories that cover the feature comprehensively.

## Implementation Decisions
Key implementation considerations including:
- Technical clarifications
- Architectural decisions
- Schema changes (if applicable)
- API contracts (if applicable)
- Specific interactions

Do NOT include specific file paths or code snippets.

## Further Notes
Any additional context or considerations."#;

#[derive(Debug, Error)]
pub enum AnthropicError {
    #[error("Missing VK_ANTHROPIC_API_KEY")]
    MissingApiKey,
    #[error("Request failed: {0}")]
    Request(String),
    #[error("Unexpected response: {0}")]
    Response(String),
    #[error("Failed to parse JSON: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f32,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: Vec<AnthropicContent<'a>>,
}

#[derive(Debug, Serialize)]
struct AnthropicContent<'a> {
    #[serde(rename = "type")]
    content_type: &'a str,
    text: &'a str,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicResponseContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicInboxResult {
    pub actionable: bool,
    pub kind: InboxItemKind,
    pub title: String,
    pub prd_markdown: String,
    #[serde(default)]
    pub context_links: Vec<String>,
}

impl AnthropicClient {
    pub fn from_env() -> Result<Self, AnthropicError> {
        let api_key = env::var("VK_ANTHROPIC_API_KEY").map_err(|_| AnthropicError::MissingApiKey)?;
        let model = env::var("VK_ANTHROPIC_MODEL")
            .ok()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| "claude-opus-4-5-20251101".to_string());
        Ok(Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        })
    }

    pub async fn classify_and_generate_prd(
        &self,
        input: &str,
    ) -> Result<AnthropicInboxResult, AnthropicError> {
        self.classify_and_generate_prd_with_template(input, DEFAULT_INBOX_PRD_TEMPLATE)
            .await
    }

    pub async fn classify_and_generate_prd_with_template(
        &self,
        input: &str,
        prd_template: &str,
    ) -> Result<AnthropicInboxResult, AnthropicError> {
        let prompt = format!(
            r#"You are an assistant that triages incoming product feedback and generates detailed PRDs for coding agents.

Return ONLY valid JSON (no markdown code fences).
JSON schema:
{{"actionable":boolean,"kind":"bug"|"feature"|"other","title":string,"prd_markdown":string,"context_links":[string]}}

Rules:
- actionable=false if there is no clear bug or feature request.
- title should be concise and human-friendly.
- prd_markdown should be a detailed PRD following this template:

{}

Guidelines for the PRD:
- Be thorough and specific
- Write for a coding LLM that will implement this feature
- Include edge cases and error handling considerations
- Make user stories extensive and cover all aspects

Input:
{}"#,
            prd_template,
            input
        );

        let request = AnthropicRequest {
            model: &self.model,
            max_tokens: 4000,
            temperature: 0.0,
            messages: vec![AnthropicMessage {
                role: "user",
                content: vec![AnthropicContent {
                    content_type: "text",
                    text: &prompt,
                }],
            }],
        };

        let send_request = || async {
            let response = self
                .client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&request)
                .send()
                .await
                .map_err(|err| AnthropicError::Request(err.to_string()))?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS
                || response.status().is_server_error()
            {
                return Err(AnthropicError::Request(format!(
                    "retryable status {}",
                    response.status()
                )));
            }

            if !response.status().is_success() {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "<empty body>".to_string());
                return Err(AnthropicError::Response(format!(
                    "status {}: {}",
                    status, body
                )));
            }

            let payload: AnthropicResponse = response
                .json()
                .await
                .map_err(|err| AnthropicError::Response(err.to_string()))?;
            Ok(payload)
        };

        let mut last_err = None;
        let payload = {
            let mut result = None;
            for attempt in 0..3 {
                match send_request().await {
                    Ok(payload) => {
                        result = Some(Ok(payload));
                        break;
                    }
                    Err(err) => {
                        last_err = Some(err);
                        if attempt < 2 {
                            let backoff_ms = 500u64 * 2u64.pow(attempt);
                            sleep(Duration::from_millis(backoff_ms)).await;
                        }
                    }
                }
            }
            result.unwrap_or_else(|| Err(last_err.unwrap()))
        }?;
        let text = payload
            .content
            .into_iter()
            .find_map(|item| match item.content_type.as_str() {
                "text" => item.text,
                _ => None,
            })
            .ok_or_else(|| AnthropicError::Response("Missing text content".to_string()))?;

        let json_text = strip_code_fences(&text);
        let parsed: AnthropicInboxResult =
            serde_json::from_str(&json_text).map_err(|err| AnthropicError::Parse(err.to_string()))?;

        Ok(parsed)
    }
}

fn strip_code_fences(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("```") {
        let trimmed = trimmed.trim_start_matches("```");
        let trimmed = trimmed.trim_start_matches("json");
        let trimmed = trimmed.trim();
        if let Some(end) = trimmed.rfind("```") {
            return trimmed[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_code_fences_handles_json_block() {
        let raw = "```json\n{\"actionable\":true,\"kind\":\"bug\",\"title\":\"Test\",\"prd_markdown\":\"Body\",\"context_links\":[]}\n```";
        let stripped = strip_code_fences(raw);
        assert!(stripped.starts_with("{\"actionable\""));
    }

    #[test]
    fn parses_anthropic_result_json() {
        let json = "{\"actionable\":true,\"kind\":\"feature\",\"title\":\"Add export\",\"prd_markdown\":\"Details\",\"context_links\":[\"https://example.com\"]}";
        let parsed: AnthropicInboxResult = serde_json::from_str(json).expect("parse json");
        assert!(parsed.actionable);
        assert!(matches!(parsed.kind, InboxItemKind::Feature));
        assert_eq!(parsed.title, "Add export");
    }
}
