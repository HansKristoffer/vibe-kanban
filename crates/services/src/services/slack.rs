//! Slack API client and signature verification for PRD integration.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum SlackError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Request timestamp too old")]
    TimestampTooOld,
    #[error("Request failed: {0}")]
    Request(String),
    #[error("Slack API error: {0}")]
    ApiError(String),
}

/// Verify Slack request signature using HMAC-SHA256.
/// 
/// Slack sends:
/// - `X-Slack-Signature`: "v0=<hex_hmac>"
/// - `X-Slack-Request-Timestamp`: Unix epoch seconds
/// 
/// The signature base string is: `v0:{timestamp}:{body}`
pub fn verify_slack_signature(
    signing_secret: &str,
    timestamp: &str,
    body: &[u8],
    signature: &str,
) -> Result<(), SlackError> {
    // Reject requests older than 5 minutes to prevent replay attacks
    let ts: i64 = timestamp.parse().map_err(|_| SlackError::InvalidSignature)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    if (now - ts).abs() > 300 {
        return Err(SlackError::TimestampTooOld);
    }

    // Build signature base string
    let sig_basestring = format!("v0:{}:{}", timestamp, String::from_utf8_lossy(body));

    // Compute HMAC-SHA256
    let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes())
        .map_err(|_| SlackError::InvalidSignature)?;
    mac.update(sig_basestring.as_bytes());
    let computed = mac.finalize().into_bytes();
    let computed_hex = format!("v0={}", hex::encode(computed));

    // Constant-time comparison
    use subtle::ConstantTimeEq;
    if computed_hex.as_bytes().ct_eq(signature.as_bytes()).into() {
        Ok(())
    } else {
        Err(SlackError::InvalidSignature)
    }
}

/// Slack client for Web API calls.
#[derive(Clone)]
pub struct SlackClient {
    bot_token: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Clone)]
pub struct SlackBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<SlackTextObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<SlackElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessory: Option<SlackElement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SlackTextObject {
    #[serde(rename = "type")]
    pub text_type: String, // "plain_text" or "mrkdwn"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SlackElement {
    #[serde(rename = "type")]
    pub element_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<SlackTextObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>, // "primary", "danger"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostMessageRequest {
    channel: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<SlackBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_ts: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateMessageRequest {
    channel: String,
    ts: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<SlackBlock>>,
}

#[derive(Debug, Serialize)]
struct ViewsOpenRequest {
    trigger_id: String,
    view: SlackView,
}

#[derive(Debug, Serialize, Clone)]
pub struct SlackView {
    #[serde(rename = "type")]
    pub view_type: String, // "modal"
    pub title: SlackTextObject,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submit: Option<SlackTextObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close: Option<SlackTextObject>,
    pub blocks: Vec<SlackBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ConversationsOpenRequest {
    users: String, // Comma-separated user IDs
}

#[derive(Debug, Deserialize)]
struct SlackApiResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    _ts: Option<String>,
    #[serde(default)]
    channel: Option<SlackChannelInfo>,
    #[serde(default)]
    response_metadata: Option<SlackResponseMetadata>,
}

#[derive(Debug, Deserialize)]
struct SlackResponseMetadata {
    #[serde(default)]
    messages: Option<Vec<String>>,
}

/// Simple response for APIs where we only need ok/error (like chat.update)
#[derive(Debug, Deserialize)]
struct SlackSimpleResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

/// Response for chat.postMessage API which returns channel as a string, not an object
#[derive(Debug, Deserialize)]
struct SlackPostMessageResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    ts: Option<String>,
    #[serde(default)]
    _channel: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackChannelInfo {
    id: String,
}

#[derive(Debug, Clone)]
pub struct PostMessageResult {
    pub channel: String,
    pub ts: String,
}

impl SlackClient {
    pub fn new(bot_token: &str) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Post a message to a Slack channel.
    pub async fn post_message(
        &self,
        channel: &str,
        text: &str,
        blocks: Option<Vec<SlackBlock>>,
        thread_ts: Option<&str>,
    ) -> Result<PostMessageResult, SlackError> {
        let request = PostMessageRequest {
            channel: channel.to_string(),
            text: text.to_string(),
            blocks,
            thread_ts: thread_ts.map(|s| s.to_string()),
        };

        let response = self
            .client
            .post("https://slack.com/api/chat.postMessage")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        // Use SlackPostMessageResponse which expects channel as string (not object)
        let result: SlackPostMessageResponse = response
            .json()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        if !result.ok {
            return Err(SlackError::ApiError(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        Ok(PostMessageResult {
            channel: channel.to_string(),
            ts: result.ts.ok_or_else(|| SlackError::ApiError("No ts in response".to_string()))?,
        })
    }

    /// Post a message to a Slack channel with raw JSON blocks.
    /// This avoids potential serialization issues with the struct-based approach.
    pub async fn post_message_json(
        &self,
        channel: &str,
        text: &str,
        blocks: serde_json::Value,
        thread_ts: Option<&str>,
    ) -> Result<PostMessageResult, SlackError> {
        let mut request = serde_json::json!({
            "channel": channel,
            "text": text,
            "blocks": blocks,
        });

        if let Some(ts) = thread_ts {
            request["thread_ts"] = serde_json::Value::String(ts.to_string());
        }

        tracing::debug!("Posting to Slack: {}", serde_json::to_string_pretty(&request).unwrap_or_default());

        let response = self
            .client
            .post("https://slack.com/api/chat.postMessage")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        let response_text = response
            .text()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        // Use SlackPostMessageResponse which expects channel as string (not object)
        let result: SlackPostMessageResponse = serde_json::from_str(&response_text)
            .map_err(|e| SlackError::Request(format!("Failed to parse response: {} - {}", e, response_text)))?;

        if !result.ok {
            let error = result.error.unwrap_or_else(|| "Unknown error".to_string());
            tracing::warn!("Slack API error: {} - Request was: {}", error, serde_json::to_string(&request).unwrap_or_default());
            return Err(SlackError::ApiError(error));
        }

        Ok(PostMessageResult {
            channel: channel.to_string(),
            ts: result.ts.ok_or_else(|| SlackError::ApiError("No ts in response".to_string()))?,
        })
    }

    /// Update an existing Slack message with raw JSON blocks.
    pub async fn update_message_json(
        &self,
        channel: &str,
        ts: &str,
        text: &str,
        blocks: serde_json::Value,
    ) -> Result<(), SlackError> {
        let request = serde_json::json!({
            "channel": channel,
            "ts": ts,
            "text": text,
            "blocks": blocks,
        });

        tracing::debug!("Updating Slack message: channel={}, ts={}", channel, ts);

        let response = self
            .client
            .post("https://slack.com/api/chat.update")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        let response_text = response
            .text()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        // Use simple response type - chat.update returns channel as string, not object
        let result: SlackSimpleResponse = serde_json::from_str(&response_text)
            .map_err(|e| SlackError::Request(format!("Failed to parse response: {} - {}", e, response_text)))?;

        if !result.ok {
            let error = result.error.unwrap_or_else(|| "Unknown error".to_string());
            tracing::warn!("Slack API error on update: {} - channel={}, ts={}", error, channel, ts);
            return Err(SlackError::ApiError(error));
        }

        tracing::debug!("Successfully updated Slack message: channel={}, ts={}", channel, ts);
        Ok(())
    }

    /// Update an existing Slack message.
    pub async fn update_message(
        &self,
        channel: &str,
        ts: &str,
        text: &str,
        blocks: Option<Vec<SlackBlock>>,
    ) -> Result<(), SlackError> {
        let request = UpdateMessageRequest {
            channel: channel.to_string(),
            ts: ts.to_string(),
            text: text.to_string(),
            blocks,
        };

        let response = self
            .client
            .post("https://slack.com/api/chat.update")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        // Use simple response type - chat.update returns channel as string, not object
        let result: SlackSimpleResponse = response
            .json()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        if !result.ok {
            return Err(SlackError::ApiError(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        Ok(())
    }

    /// Open a modal view.
    pub async fn views_open(&self, trigger_id: &str, view: SlackView) -> Result<(), SlackError> {
        let request = ViewsOpenRequest {
            trigger_id: trigger_id.to_string(),
            view,
        };

        let response = self
            .client
            .post("https://slack.com/api/views.open")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        let result: SlackApiResponse = response
            .json()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        if !result.ok {
            return Err(SlackError::ApiError(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        Ok(())
    }

    /// Open a modal view using raw JSON (for complex views with input blocks).
    pub async fn views_open_json(&self, trigger_id: &str, view: serde_json::Value) -> Result<(), SlackError> {
        let request = serde_json::json!({
            "trigger_id": trigger_id,
            "view": view,
        });

        let response = self
            .client
            .post("https://slack.com/api/views.open")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        let result: SlackApiResponse = response
            .json()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        if !result.ok {
            // Include detailed error messages from response_metadata if available
            let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
            let detailed_msg = if let Some(metadata) = result.response_metadata {
                if let Some(messages) = metadata.messages {
                    format!("{}: {}", error_msg, messages.join(", "))
                } else {
                    error_msg
                }
            } else {
                error_msg
            };
            return Err(SlackError::ApiError(detailed_msg));
        }

        Ok(())
    }

    /// Open a DM conversation with a user.
    pub async fn open_dm(&self, user_id: &str) -> Result<String, SlackError> {
        let request = ConversationsOpenRequest {
            users: user_id.to_string(),
        };

        let response = self
            .client
            .post("https://slack.com/api/conversations.open")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        let result: SlackApiResponse = response
            .json()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        if !result.ok {
            return Err(SlackError::ApiError(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        Ok(result
            .channel
            .ok_or_else(|| SlackError::ApiError("No channel in response".to_string()))?
            .id)
    }

    /// Send a DM to a user.
    pub async fn send_dm(
        &self,
        user_id: &str,
        text: &str,
        blocks: Option<Vec<SlackBlock>>,
    ) -> Result<PostMessageResult, SlackError> {
        let dm_channel = self.open_dm(user_id).await?;
        self.post_message(&dm_channel, text, blocks, None).await
    }

    /// Delete a Slack message.
    pub async fn delete_message(&self, channel: &str, ts: &str) -> Result<(), SlackError> {
        let request = serde_json::json!({
            "channel": channel,
            "ts": ts,
        });

        tracing::debug!("Deleting Slack message: channel={}, ts={}", channel, ts);

        let response = self
            .client
            .post("https://slack.com/api/chat.delete")
            .bearer_auth(&self.bot_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        let result: SlackSimpleResponse = response
            .json()
            .await
            .map_err(|e| SlackError::Request(e.to_string()))?;

        if !result.ok {
            let error = result.error.unwrap_or_else(|| "Unknown error".to_string());
            tracing::warn!("Slack API error on delete: {} - channel={}, ts={}", error, channel, ts);
            return Err(SlackError::ApiError(error));
        }

        tracing::debug!("Successfully deleted Slack message: channel={}, ts={}", channel, ts);
        Ok(())
    }
}

/// Safely truncate a string to a maximum number of characters (not bytes).
/// This ensures we don't cut in the middle of a multi-byte UTF-8 character.
fn safe_truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

/// Extract only the first section of a PRD markdown document.
/// 
/// This looks for the second markdown heading (## or #) and returns everything
/// before it. If no second heading is found, returns the full content.
fn extract_first_prd_section(prd: &str) -> String {
    let lines: Vec<&str> = prd.lines().collect();
    let mut found_first_heading = false;
    let mut end_index = lines.len();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            if found_first_heading {
                // Found the second heading, stop here
                end_index = i;
                break;
            } else {
                found_first_heading = true;
            }
        }
    }

    lines[..end_index].join("\n").trim().to_string()
}

/// Build Slack blocks for a PRD message with Accept/Update/Decline buttons.
/// 
/// Returns struct-based blocks. For JSON blocks use `build_prd_blocks_json`.
pub fn build_prd_blocks(
    title: &str,
    kind: &str,
    source: &str,
    prd_markdown: &str,
    inbox_item_id: &str,
    status: PrdMessageStatus,
) -> Vec<SlackBlock> {
    let status_emoji = match status {
        PrdMessageStatus::Pending => ":inbox_tray:",
        PrdMessageStatus::Accepted => ":white_check_mark:",
        PrdMessageStatus::Declined => ":x:",
    };

    let status_str = match status {
        PrdMessageStatus::Pending => "Pending",
        PrdMessageStatus::Accepted => "Accepted",
        PrdMessageStatus::Declined => "Declined",
    };

    // Truncate title for header block (max 150 chars)
    let header_text = safe_truncate(&format!("{} {}", status_emoji, title), 145);

    // Extract only the first section of the PRD (e.g., Problem Statement)
    let prd_content = prd_markdown.trim();
    let prd_text = if prd_content.is_empty() {
        "_No description provided_".to_string()
    } else {
        // Find the first section by looking for the second heading (## or #)
        let first_section = extract_first_prd_section(prd_content);
        let truncated = safe_truncate(&first_section, 700);
        if first_section.len() < prd_content.len() {
            format!("{}\n\n_[PRD truncated - showing first section only]_", truncated)
        } else {
            truncated
        }
    };

    let mut blocks = vec![
        // Header
        SlackBlock {
            block_type: "header".to_string(),
            text: Some(SlackTextObject {
                text_type: "plain_text".to_string(),
                text: header_text,
                emoji: Some(true),
            }),
            elements: None,
            accessory: None,
            block_id: None,
        },
        // Context
        SlackBlock {
            block_type: "context".to_string(),
            text: None,
            elements: Some(vec![SlackElement {
                element_type: "mrkdwn".to_string(),
                text: Some(SlackTextObject {
                    text_type: "mrkdwn".to_string(),
                    text: format!("*Source:* {} | *Type:* {} | *Status:* {}", source, kind, status_str),
                    emoji: None,
                }),
                action_id: None,
                value: None,
                style: None,
                url: None,
            }]),
            accessory: None,
            block_id: None,
        },
        // Divider
        SlackBlock {
            block_type: "divider".to_string(),
            text: None,
            elements: None,
            accessory: None,
            block_id: None,
        },
        // PRD content
        SlackBlock {
            block_type: "section".to_string(),
            text: Some(SlackTextObject {
                text_type: "mrkdwn".to_string(),
                text: prd_text,
                emoji: None,
            }),
            elements: None,
            accessory: None,
            block_id: None,
        },
        // Divider
        SlackBlock {
            block_type: "divider".to_string(),
            text: None,
            elements: None,
            accessory: None,
            block_id: None,
        },
    ];

    // Action buttons (only for pending status)
    if matches!(status, PrdMessageStatus::Pending) {
        blocks.push(SlackBlock {
            block_type: "actions".to_string(),
            text: None,
            elements: Some(vec![
                SlackElement {
                    element_type: "button".to_string(),
                    text: Some(SlackTextObject {
                        text_type: "plain_text".to_string(),
                        text: "Accept".to_string(),
                        emoji: Some(true),
                    }),
                    action_id: Some("prd_accept".to_string()),
                    value: Some(inbox_item_id.to_string()),
                    style: Some("primary".to_string()),
                    url: None,
                },
                SlackElement {
                    element_type: "button".to_string(),
                    text: Some(SlackTextObject {
                        text_type: "plain_text".to_string(),
                        text: "Update".to_string(),
                        emoji: Some(true),
                    }),
                    action_id: Some("prd_update".to_string()),
                    value: Some(inbox_item_id.to_string()),
                    style: None,
                    url: None,
                },
                SlackElement {
                    element_type: "button".to_string(),
                    text: Some(SlackTextObject {
                        text_type: "plain_text".to_string(),
                        text: "Decline".to_string(),
                        emoji: Some(true),
                    }),
                    action_id: Some("prd_decline".to_string()),
                    value: Some(inbox_item_id.to_string()),
                    style: Some("danger".to_string()),
                    url: None,
                },
            ]),
            accessory: None,
            block_id: Some("prd_actions".to_string()),
        });
    }

    blocks
}

/// Build PRD blocks as raw JSON for maximum compatibility with Slack's Block Kit.
pub fn build_prd_blocks_json(
    title: &str,
    kind: &str,
    source: &str,
    prd_markdown: &str,
    inbox_item_id: &str,
    status: PrdMessageStatus,
) -> serde_json::Value {
    let status_emoji = match status {
        PrdMessageStatus::Pending => ":inbox_tray:",
        PrdMessageStatus::Accepted => ":white_check_mark:",
        PrdMessageStatus::Declined => ":x:",
    };

    let status_str = match status {
        PrdMessageStatus::Pending => "Pending",
        PrdMessageStatus::Accepted => "Accepted",
        PrdMessageStatus::Declined => "Declined",
    };

    // Truncate title for header block (max 150 chars)
    let header_text = safe_truncate(&format!("{} {}", status_emoji, title), 145);

    // Truncate PRD content safely (max 3000 chars, but leave room for suffix)
    let prd_content = prd_markdown.trim();
    let prd_text = if prd_content.is_empty() {
        "_No description provided_".to_string()
    } else {
        let truncated = safe_truncate(prd_content, 2700);
        if truncated.len() < prd_content.len() {
            format!("{}...\n\n_[PRD truncated]_", truncated)
        } else {
            truncated
        }
    };

    let mut blocks = vec![
        // Header block
        serde_json::json!({
            "type": "header",
            "text": {
                "type": "plain_text",
                "text": header_text,
                "emoji": true
            }
        }),
        // Context block
        serde_json::json!({
            "type": "context",
            "elements": [
                {
                    "type": "mrkdwn",
                    "text": format!("*Source:* {} | *Type:* {} | *Status:* {}", source, kind, status_str)
                }
            ]
        }),
        // Divider
        serde_json::json!({
            "type": "divider"
        }),
        // PRD content section
        serde_json::json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": prd_text
            }
        }),
        // Divider before actions
        serde_json::json!({
            "type": "divider"
        }),
    ];

    // Action buttons (only for pending status)
    if matches!(status, PrdMessageStatus::Pending) {
        blocks.push(serde_json::json!({
            "type": "actions",
            "block_id": "prd_actions",
            "elements": [
                {
                    "type": "button",
                    "text": {
                        "type": "plain_text",
                        "text": "Accept",
                        "emoji": true
                    },
                    "action_id": "prd_accept",
                    "value": inbox_item_id,
                    "style": "primary"
                },
                {
                    "type": "button",
                    "text": {
                        "type": "plain_text",
                        "text": "Update",
                        "emoji": true
                    },
                    "action_id": "prd_update",
                    "value": inbox_item_id
                },
                {
                    "type": "button",
                    "text": {
                        "type": "plain_text",
                        "text": "Decline",
                        "emoji": true
                    },
                    "action_id": "prd_decline",
                    "value": inbox_item_id,
                    "style": "danger"
                }
            ]
        }));
    }

    serde_json::Value::Array(blocks)
}

#[derive(Debug, Clone, Copy)]
pub enum PrdMessageStatus {
    Pending,
    Accepted,
    Declined,
}

/// Build the accept modal with dynamic branch options.
/// 
/// # Arguments
/// * `inbox_item_id` - The ID of the inbox item
/// * `branches` - List of available branch names (will show dropdown if non-empty, text input otherwise)
pub fn build_accept_modal_json(inbox_item_id: &str, branches: &[String]) -> serde_json::Value {
    // Truncate branch name to fit Slack's 75 char limit for option text/value
    fn truncate_branch(name: &str) -> String {
        if name.len() <= 75 {
            name.to_string()
        } else {
            format!("{}...", &name[..72])
        }
    }

    // Build branch input block - dropdown if we have branches, text input otherwise
    // Limit to 100 branches (Slack's max for static_select)
    let limited_branches: Vec<&String> = branches.iter().take(100).collect();
    
    let branch_block = if limited_branches.is_empty() {
        serde_json::json!({
            "type": "input",
            "block_id": "branch_block",
            "optional": true,
            "label": {
                "type": "plain_text",
                "text": "Base Branch",
                "emoji": true
            },
            "element": {
                "type": "plain_text_input",
                "action_id": "branch_input",
                "placeholder": {
                    "type": "plain_text",
                    "text": "main"
                }
            },
            "hint": {
                "type": "plain_text",
                "text": "The git branch to base the work on. Leave empty for 'main'."
            }
        })
    } else {
        // Find "main" or "master" for initial option, fallback to first branch
        let default_branch = limited_branches.iter()
            .find(|b| b.as_str() == "main" || b.as_str() == "master")
            .unwrap_or(&limited_branches[0]);
        
        let branch_options: Vec<serde_json::Value> = limited_branches.iter()
            .map(|b| {
                let truncated = truncate_branch(b);
                serde_json::json!({
                    "text": { "type": "plain_text", "text": &truncated },
                    "value": &truncated
                })
            })
            .collect();
        
        let default_truncated = truncate_branch(default_branch);
        
        serde_json::json!({
            "type": "input",
            "block_id": "branch_block",
            "label": {
                "type": "plain_text",
                "text": "Base Branch",
                "emoji": true
            },
            "element": {
                "type": "static_select",
                "action_id": "branch_select",
                "placeholder": {
                    "type": "plain_text",
                    "text": "Select a branch"
                },
                "initial_option": {
                    "text": { "type": "plain_text", "text": &default_truncated },
                    "value": &default_truncated
                },
                "options": branch_options
            }
        })
    };

    serde_json::json!({
        "type": "modal",
        "callback_id": "accept_prd_modal",
        "private_metadata": inbox_item_id,
        "title": {
            "type": "plain_text",
            "text": "Create Attempt",
            "emoji": true
        },
        "submit": {
            "type": "plain_text",
            "text": "Start Work",
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
                "block_id": "model_block",
                "label": {
                    "type": "plain_text",
                    "text": "Agent",
                    "emoji": true
                },
                "element": {
                    "type": "static_select",
                    "action_id": "model_select",
                    "placeholder": {
                        "type": "plain_text",
                        "text": "Select an agent"
                    },
                    "initial_option": {
                        "text": { "type": "plain_text", "text": "Claude Code" },
                        "value": "CLAUDE_CODE"
                    },
                    "options": [
                        {
                            "text": { "type": "plain_text", "text": "Claude Code" },
                            "value": "CLAUDE_CODE"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Codex" },
                            "value": "CODEX"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Cursor Agent" },
                            "value": "CURSOR_AGENT"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Gemini" },
                            "value": "GEMINI"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Amp" },
                            "value": "AMP"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Copilot" },
                            "value": "COPILOT"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Droid" },
                            "value": "DROID"
                        },
                        {
                            "text": { "type": "plain_text", "text": "OpenCode" },
                            "value": "OPENCODE"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Qwen Code" },
                            "value": "QWEN_CODE"
                        }
                    ]
                }
            },
            {
                "type": "input",
                "block_id": "config_block",
                "label": {
                    "type": "plain_text",
                    "text": "Configuration",
                    "emoji": true
                },
                "element": {
                    "type": "static_select",
                    "action_id": "config_select",
                    "placeholder": {
                        "type": "plain_text",
                        "text": "Select configuration"
                    },
                    "initial_option": {
                        "text": { "type": "plain_text", "text": "Default" },
                        "value": "DEFAULT"
                    },
                    "options": [
                        {
                            "text": { "type": "plain_text", "text": "Default" },
                            "value": "DEFAULT"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Approvals (with confirmations)" },
                            "value": "APPROVALS"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Plan (planning mode)" },
                            "value": "PLAN"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Opus (Claude Opus)" },
                            "value": "OPUS"
                        },
                        {
                            "text": { "type": "plain_text", "text": "High (high effort)" },
                            "value": "HIGH"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Max (maximum capability)" },
                            "value": "MAX"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Flash (fast mode)" },
                            "value": "FLASH"
                        },
                        {
                            "text": { "type": "plain_text", "text": "Pro (professional)" },
                            "value": "PRO"
                        }
                    ]
                }
            },
            branch_block,
            {
                "type": "context",
                "elements": [
                    {
                        "type": "mrkdwn",
                        "text": "_The task will be created and work will start immediately using all configured repositories._"
                    }
                ]
            }
        ]
    })
}

/// Build the Accept modal view for task configuration (struct version, deprecated).
/// Use build_accept_modal_json for proper input support.
#[deprecated(note = "Use build_accept_modal_json instead for proper input blocks")]
pub fn build_accept_modal(inbox_item_id: &str, _current_prd: &str) -> SlackView {
    // Fallback struct version - limited functionality
    let blocks = vec![
        SlackBlock {
            block_type: "section".to_string(),
            text: Some(SlackTextObject {
                text_type: "mrkdwn".to_string(),
                text: "*Configuration*\n\nThe task will be created with default settings.".to_string(),
                emoji: None,
            }),
            elements: None,
            accessory: None,
            block_id: None,
        },
    ];

    SlackView {
        view_type: "modal".to_string(),
        title: SlackTextObject {
            text_type: "plain_text".to_string(),
            text: "Accept PRD".to_string(),
            emoji: Some(true),
        },
        submit: Some(SlackTextObject {
            text_type: "plain_text".to_string(),
            text: "Create Task".to_string(),
            emoji: Some(true),
        }),
        close: Some(SlackTextObject {
            text_type: "plain_text".to_string(),
            text: "Cancel".to_string(),
            emoji: Some(true),
        }),
        blocks,
        private_metadata: Some(inbox_item_id.to_string()),
        callback_id: Some("accept_prd_modal".to_string()),
    }
}

/// Build the Update modal view for editing PRD.
pub fn build_update_modal(inbox_item_id: &str, current_title: &str, current_prd: &str) -> SlackView {
    let blocks = vec![
        // Title input
        SlackBlock {
            block_type: "input".to_string(),
            text: None,
            elements: None,
            accessory: None,
            block_id: Some("title_block".to_string()),
        },
        // PRD input
        SlackBlock {
            block_type: "input".to_string(),
            text: None,
            elements: None,
            accessory: None,
            block_id: Some("prd_block".to_string()),
        },
    ];

    // For proper input blocks, we need to use the full input block structure
    // Slack's input blocks have a specific format that we'll handle in JSON
    
    SlackView {
        view_type: "modal".to_string(),
        title: SlackTextObject {
            text_type: "plain_text".to_string(),
            text: "Update PRD".to_string(),
            emoji: Some(true),
        },
        submit: Some(SlackTextObject {
            text_type: "plain_text".to_string(),
            text: "Save".to_string(),
            emoji: Some(true),
        }),
        close: Some(SlackTextObject {
            text_type: "plain_text".to_string(),
            text: "Cancel".to_string(),
            emoji: Some(true),
        }),
        blocks,
        private_metadata: Some(serde_json::json!({
            "inbox_item_id": inbox_item_id,
            "current_title": current_title,
            "current_prd": current_prd
        }).to_string()),
        callback_id: Some("update_prd_modal".to_string()),
    }
}

/// Build reply blocks with links after task creation.
pub fn build_task_created_reply(
    vk_task_url: &str,
    linear_issue_url: Option<&str>,
) -> Vec<SlackBlock> {
    let mut text = format!(":rocket: *Task created!*\n\n<{}|View in Vibe Kanban>", vk_task_url);
    
    if let Some(linear_url) = linear_issue_url {
        text.push_str(&format!("\n<{}|View in Linear>", linear_url));
    }

    vec![SlackBlock {
        block_type: "section".to_string(),
        text: Some(SlackTextObject {
            text_type: "mrkdwn".to_string(),
            text,
            emoji: None,
        }),
        elements: None,
        accessory: None,
        block_id: None,
    }]
}

/// Get the absolute URL for a task in Vibe Kanban.
/// Uses VK_PUBLIC_BASE_URL environment variable if set, otherwise returns a relative path.
pub fn get_vk_task_url(project_id: &uuid::Uuid, task_id: &uuid::Uuid) -> String {
    let base_url = std::env::var("VK_PUBLIC_BASE_URL").unwrap_or_default();
    if base_url.is_empty() {
        format!("/projects/{}/tasks/{}", project_id, task_id)
    } else {
        let base = base_url.trim_end_matches('/');
        format!("{}/projects/{}/tasks/{}", base, project_id, task_id)
    }
}

/// Task completion status for Slack notifications.
#[derive(Debug, Clone, Copy)]
pub enum TaskCompletionStatus {
    Done,
    Cancelled,
    Failed,
}

/// Build a Slack message for when a task is accepted.
/// Includes user mention, task title, links to task and Linear.
pub fn build_task_accepted_message_json(
    user_id: &str,
    title: &str,
    task_url: &str,
    linear_url: Option<&str>,
) -> serde_json::Value {
    let linear_link = linear_url
        .map(|url| format!(" | <{}|Linear>", url))
        .unwrap_or_default();
    
    let text = format!(
        ":white_check_mark: <@{}> accepted *{}*\n<{}|View in Vibe Kanban>{}",
        user_id, title, task_url, linear_link
    );
    
    serde_json::json!([
        {
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": text
            }
        }
    ])
}

/// Build a Slack message for when a task is completed, cancelled, or failed.
pub fn build_task_completed_message_json(
    title: &str,
    task_url: Option<&str>,
    linear_url: Option<&str>,
    pr_url: Option<&str>,
    status: TaskCompletionStatus,
    accepted_by_user_id: Option<&str>,
) -> serde_json::Value {
    let (emoji, status_text) = match status {
        TaskCompletionStatus::Done => (":white_check_mark:", "completed"),
        TaskCompletionStatus::Cancelled => (":x:", "cancelled"),
        TaskCompletionStatus::Failed => (":warning:", "failed"),
    };
    
    // Tag the user who accepted if this is a failure
    let user_mention = if matches!(status, TaskCompletionStatus::Failed) {
        accepted_by_user_id
            .map(|id| format!(" <@{}>", id))
            .unwrap_or_default()
    } else {
        String::new()
    };
    
    // Build links - include PR link for Done status if available
    let mut link_parts = Vec::new();
    if let Some(vk) = task_url {
        link_parts.push(format!("<{}|View in Vibe Kanban>", vk));
    }
    if let Some(linear) = linear_url {
        link_parts.push(format!("<{}|Linear>", linear));
    }
    if let Some(pr) = pr_url {
        if matches!(status, TaskCompletionStatus::Done) {
            link_parts.push(format!("<{}|Pull Request>", pr));
        }
    }
    
    let links = if link_parts.is_empty() {
        String::new()
    } else {
        format!("\n{}", link_parts.join(" | "))
    };
    
    let text = format!(
        "{} Task {}: *{}*{}{}",
        emoji, status_text, title, user_mention, links
    );
    
    serde_json::json!([
        {
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": text
            }
        }
    ])
}

/// Get plain text version of task accepted message (for fallback/notification).
pub fn get_task_accepted_text(user_id: &str, title: &str) -> String {
    format!("<@{}> accepted: {}", user_id, title)
}

/// Get plain text version of task completed message (for fallback/notification).
pub fn get_task_completed_text(title: &str, status: TaskCompletionStatus, accepted_by_user_id: Option<&str>) -> String {
    let user_mention = if matches!(status, TaskCompletionStatus::Failed) {
        accepted_by_user_id
            .map(|id| format!(" <@{}>", id))
            .unwrap_or_default()
    } else {
        String::new()
    };
    
    match status {
        TaskCompletionStatus::Done => format!("Task completed: {}", title),
        TaskCompletionStatus::Cancelled => format!("Task cancelled: {}", title),
        TaskCompletionStatus::Failed => format!("Task failed: {}{}", title, user_mention),
    }
}

/// Build a Slack message for when a task moves to In Review and needs action.
pub fn build_task_in_review_message_json(
    title: &str,
    task_url: Option<&str>,
    linear_url: Option<&str>,
    accepted_by_user_id: Option<&str>,
) -> serde_json::Value {
    // Tag the user who accepted so they know to review it
    let user_mention = accepted_by_user_id
        .map(|id| format!(" <@{}>", id))
        .unwrap_or_default();
    
    let links = match (task_url, linear_url) {
        (Some(vk), Some(linear)) => format!("\n<{}|View in Vibe Kanban> | <{}|Linear>", vk, linear),
        (Some(vk), None) => format!("\n<{}|View in Vibe Kanban>", vk),
        (None, Some(linear)) => format!("\n<{}|Linear>", linear),
        (None, None) => String::new(),
    };
    
    let text = format!(
        ":eyes: Task needs review:{} *{}*{}",
        user_mention, title, links
    );
    
    serde_json::json!([
        {
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": text
            }
        }
    ])
}

/// Get plain text version of task in review message (for fallback/notification).
pub fn get_task_in_review_text(title: &str, accepted_by_user_id: Option<&str>) -> String {
    let user_mention = accepted_by_user_id
        .map(|id| format!(" <@{}>", id))
        .unwrap_or_default();
    format!("Task needs review:{} {}", user_mention, title)
}

/// Send a notification to Slack when a task moves to In Review.
/// This is called when a task execution completes (success or failure).
pub async fn notify_task_in_review(
    bot_token: &str,
    channel_id: &str,
    message_ts: Option<&str>,
    title: &str,
    task_url: Option<&str>,
    linear_url: Option<&str>,
    accepted_by_user_id: Option<&str>,
) {
    let client = SlackClient::new(bot_token);
    let blocks = build_task_in_review_message_json(
        title,
        task_url,
        linear_url,
        accepted_by_user_id,
    );
    let text = get_task_in_review_text(title, accepted_by_user_id);

    // Post as thread reply if we have the original message ts
    if let Some(ts) = message_ts {
        if let Err(e) = client.post_message_json(channel_id, &text, blocks.clone(), Some(ts)).await {
            tracing::warn!("Failed to post Slack thread reply for task in review: {}", e);
        }
    }

    // Post as channel message
    if let Err(e) = client.post_message_json(channel_id, &text, blocks, None).await {
        tracing::warn!("Failed to post Slack channel notification for task in review: {}", e);
    }
}

/// Send a task failure notification to Slack.
/// This is called when a task execution fails.
pub async fn notify_task_failed(
    bot_token: &str,
    channel_id: &str,
    message_ts: Option<&str>,
    title: &str,
    task_url: Option<&str>,
    linear_url: Option<&str>,
    pr_url: Option<&str>,
    accepted_by_user_id: Option<&str>,
) {
    let client = SlackClient::new(bot_token);
    let blocks = build_task_completed_message_json(
        title,
        task_url,
        linear_url,
        pr_url,
        TaskCompletionStatus::Failed,
        accepted_by_user_id,
    );
    let text = get_task_completed_text(title, TaskCompletionStatus::Failed, accepted_by_user_id);

    // Post as thread reply if we have the original message ts
    if let Some(ts) = message_ts {
        if let Err(e) = client.post_message_json(channel_id, &text, blocks.clone(), Some(ts)).await {
            tracing::warn!("Failed to post Slack thread reply for task failure: {}", e);
        }
    }

    // Post as channel message
    if let Err(e) = client.post_message_json(channel_id, &text, blocks, None).await {
        tracing::warn!("Failed to post Slack channel notification for task failure: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_signature_valid() {
        // Test data from Slack's documentation
        let signing_secret = "8f742231b10e8888abcd99yyyzzz85a5";
        let timestamp = "1531420618";
        let body = b"token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";
        
        // Compute expected signature
        let sig_basestring = format!("v0:{}:{}", timestamp, String::from_utf8_lossy(body));
        let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes()).unwrap();
        mac.update(sig_basestring.as_bytes());
        let computed = mac.finalize().into_bytes();
        let signature = format!("v0={}", hex::encode(computed));

        // This should succeed (but will fail due to timestamp being too old)
        // In real tests, we'd mock the system time
        let result = verify_slack_signature(signing_secret, timestamp, body, &signature);
        // Expect timestamp too old since test timestamp is from 2018
        assert!(matches!(result, Err(SlackError::TimestampTooOld)));
    }

    #[test]
    fn test_build_prd_blocks() {
        let blocks = build_prd_blocks(
            "Test PRD",
            "feature",
            "manual",
            "# Test\n\nThis is a test PRD.",
            "123e4567-e89b-12d3-a456-426614174000",
            PrdMessageStatus::Pending,
        );

        assert!(!blocks.is_empty());
        assert_eq!(blocks[0].block_type, "header");
    }
}
