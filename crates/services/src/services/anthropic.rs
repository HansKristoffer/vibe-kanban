//! Shared types for PRD generation.
//!
//! This module contains shared types used by the Claude Code PRD service.
//! The actual PRD generation is now handled by `claude_code_prd.rs` which uses
//! Claude Code CLI with codebase context.

use db::models::inbox_item::InboxItemKind;
use serde::Deserialize;

/// Default PRD template for inbox item generation
pub const DEFAULT_INBOX_PRD_TEMPLATE: &str = r#"## Problem
What issue or need is being addressed?

## Solution
How will this be solved? Describe the approach briefly.

## Requirements
What needs to be true when this is complete? List as bullet points:
- Requirement 1
- Requirement 2

## Example (optional)
A brief example of expected behavior.

## Context (optional)
Where this fits in the existing system or related features.

## Technical Notes (optional)
Any relevant technical context for implementation."#;

/// Result from PRD generation (used by Claude Code CLI)
#[derive(Debug, Deserialize)]
pub struct AnthropicInboxResult {
    pub actionable: bool,
    pub kind: InboxItemKind,
    pub title: String,
    pub prd_markdown: String,
    #[serde(default)]
    pub context_links: Vec<String>,
    /// Whether to recommend using Ralph mode (multi-iteration development)
    /// for larger tasks that benefit from being split into multiple steps
    #[serde(default)]
    pub recommend_ralph: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inbox_result_json() {
        let json = r#"{"actionable":true,"kind":"feature","title":"Add export","prd_markdown":"Details","context_links":["https://example.com"]}"#;
        let parsed: AnthropicInboxResult = serde_json::from_str(json).expect("parse json");
        assert!(parsed.actionable);
        assert!(matches!(parsed.kind, InboxItemKind::Feature));
        assert_eq!(parsed.title, "Add export");
        assert!(!parsed.recommend_ralph); // defaults to false
    }

    #[test]
    fn parses_inbox_result_with_ralph() {
        let json = r#"{"actionable":true,"kind":"feature","title":"Build auth system","prd_markdown":"Details","context_links":[],"recommend_ralph":true}"#;
        let parsed: AnthropicInboxResult = serde_json::from_str(json).expect("parse json");
        assert!(parsed.recommend_ralph);
    }
}
