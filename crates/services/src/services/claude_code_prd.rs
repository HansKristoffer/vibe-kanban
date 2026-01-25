//! Claude Code CLI-based PRD generation service.
//!
//! This service uses Claude Code in non-interactive mode (`-p` flag) to generate PRDs
//! with full codebase context. The agent runs in the project's repository directory
//! after checking out the main branch.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info, warn};

// Re-export shared types from anthropic module
pub use super::anthropic::{AnthropicInboxResult, DEFAULT_INBOX_PRD_TEMPLATE};

/// The Claude Code CLI command - use same version as the task executor
const CLAUDE_CODE_COMMAND: &str = "npx";
const CLAUDE_CODE_PACKAGE: &str = "-y";
const CLAUDE_CODE_PKG_NAME: &str = "@anthropic-ai/claude-code@2.1.12";

/// Timeout for Claude Code CLI execution (3 minutes)
const CLAUDE_CODE_TIMEOUT_SECS: u64 = 180;

#[derive(Debug, Error)]
pub enum ClaudeCodePrdError {
    #[error("No repository configured for project")]
    NoRepository,
    #[error("Repository path does not exist: {0}")]
    RepositoryNotFound(String),
    #[error("Failed to checkout main branch: {0}")]
    GitCheckoutFailed(String),
    #[error("Claude Code CLI execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Claude Code CLI not available: {0}")]
    CliNotAvailable(String),
    #[error("Failed to parse Claude Code output: {0}")]
    ParseError(String),
    #[error("Claude Code returned no result")]
    EmptyResponse,
    #[error("Claude Code CLI execution timed out after {0} seconds")]
    Timeout(u64),
}

/// Claude Code PRD generation service
pub struct ClaudeCodePrdService;

impl ClaudeCodePrdService {
    /// Generate a PRD using Claude Code CLI with codebase context.
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    /// * `input` - The user's input text (feature request, bug report, etc.)
    /// * `prd_template` - The PRD template to use
    ///
    /// # Returns
    /// An `AnthropicInboxResult` containing the classification and PRD
    pub async fn classify_and_generate_prd(
        repo_path: &Path,
        input: &str,
        prd_template: &str,
    ) -> Result<AnthropicInboxResult, ClaudeCodePrdError> {
        // Validate repository path exists
        if !repo_path.exists() {
            return Err(ClaudeCodePrdError::RepositoryNotFound(
                repo_path.display().to_string(),
            ));
        }

        // Checkout main branch (try main first, then master)
        Self::checkout_main_branch(repo_path).await?;

        // Build the prompt
        let prompt = Self::build_prompt(input, prd_template);

        // Execute Claude Code CLI
        let output = Self::execute_claude_code(repo_path, &prompt).await?;

        // Parse the output
        Self::parse_output(&output)
    }

    /// Checkout the main branch (tries 'main' first, then 'master')
    async fn checkout_main_branch(repo_path: &Path) -> Result<(), ClaudeCodePrdError> {
        debug!("Checking out main branch in {:?}", repo_path);

        // First, try to checkout 'main'
        let main_result = Command::new("git")
            .args(["checkout", "main"])
            .current_dir(repo_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await;

        match main_result {
            Ok(output) if output.status.success() => {
                debug!("Successfully checked out 'main' branch");
                return Ok(());
            }
            Ok(_) => {
                debug!("'main' branch not found, trying 'master'");
            }
            Err(e) => {
                warn!("Git command failed: {}", e);
            }
        }

        // Try 'master' as fallback
        let master_result = Command::new("git")
            .args(["checkout", "master"])
            .current_dir(repo_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await;

        match master_result {
            Ok(output) if output.status.success() => {
                debug!("Successfully checked out 'master' branch");
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(ClaudeCodePrdError::GitCheckoutFailed(format!(
                    "Neither 'main' nor 'master' branch found: {}",
                    stderr
                )))
            }
            Err(e) => Err(ClaudeCodePrdError::GitCheckoutFailed(e.to_string())),
        }
    }

    /// Build the prompt for Claude Code
    fn build_prompt(input: &str, prd_template: &str) -> String {
        format!(
            r#"Generate a PRD for this feature request or bug report.

Return ONLY valid JSON (no markdown code fences, no explanatory text).
JSON schema:
{{"actionable":boolean,"kind":"bug"|"feature"|"other","title":string,"prd_markdown":string,"context_links":[],"recommend_ralph":boolean}}

Rules:
- actionable=false if there is no clear bug or feature request.
- kind: "bug" for bugs, "feature" for new features, "other" for unclear.
- title should be concise and human-friendly.
- prd_markdown should be a detailed PRD following this template:

{prd_template}

- recommend_ralph: Set to true if this is a larger task that should be split into multiple implementation steps. Ralph mode runs multiple Claude Code sessions, each completing one checklist item. Recommend Ralph when:
  * The task involves 3+ distinct implementation steps
  * Changes span multiple files or components
  * The work can be naturally broken into independent subtasks
  * A single session would struggle to complete everything
  Set to false for simple bug fixes, small features, or single-file changes.

Guidelines:
- Be clear and concise
- Write for a coding LLM that will implement this
- Focus on what needs to be built, not how
- If recommend_ralph is true, format the Requirements section as a checklist with "- [ ]" items

Input:
{input}"#
        )
    }

    /// Execute Claude Code CLI in non-interactive mode
    async fn execute_claude_code(
        repo_path: &Path,
        prompt: &str,
    ) -> Result<String, ClaudeCodePrdError> {
        info!("Executing Claude Code CLI in {:?} (timeout: {}s)", repo_path, CLAUDE_CODE_TIMEOUT_SECS);

        // Check if local auth exists
        if let Some(home) = dirs::home_dir() {
            let auth_file = home.join(".claude.json");
            if auth_file.exists() {
                debug!("Claude Code auth file found at {:?}", auth_file);
            } else {
                warn!("Claude Code auth file not found at {:?}. CLI may not be authenticated.", auth_file);
            }
        }

        let args = [
            CLAUDE_CODE_PACKAGE,
            CLAUDE_CODE_PKG_NAME,
            "-p",
            prompt,
            "--dangerously-skip-permissions",
            "--output-format",
            "json",
            "--max-turns",
            "1",
        ];
        
        debug!(
            "Running: {} {} (in {:?})",
            CLAUDE_CODE_COMMAND,
            args[..3].join(" ") + " [prompt] " + &args[4..].join(" "),
            repo_path
        );

        // Use login shell to ensure PATH and other env vars are set correctly
        let prompt_escaped = prompt.replace("'", "'\\''");
        let shell_cmd = format!(
            "{} {} {} -p '{}' --dangerously-skip-permissions --output-format json --max-turns 1",
            CLAUDE_CODE_COMMAND,
            CLAUDE_CODE_PACKAGE,
            CLAUDE_CODE_PKG_NAME,
            prompt_escaped
        );
        
        debug!("Shell command length: {} chars", shell_cmd.len());

        // Use zsh login shell to get proper PATH (including nvm/node)
        let mut cmd = Command::new("/bin/zsh");
        cmd.args(["-l", "-c", &shell_cmd])
            .current_dir(repo_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let command_future = cmd.output();

        let output = timeout(Duration::from_secs(CLAUDE_CODE_TIMEOUT_SECS), command_future)
            .await
            .map_err(|_| {
                warn!("Claude Code CLI timed out after {} seconds", CLAUDE_CODE_TIMEOUT_SECS);
                ClaudeCodePrdError::Timeout(CLAUDE_CODE_TIMEOUT_SECS)
            })?
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ClaudeCodePrdError::CliNotAvailable(
                        "npx command not found. Please ensure Node.js is installed.".to_string(),
                    )
                } else {
                    ClaudeCodePrdError::ExecutionFailed(e.to_string())
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            warn!(
                "Claude Code CLI failed with status {:?}. stderr: {}, stdout: {}",
                output.status.code(),
                stderr,
                stdout
            );
            return Err(ClaudeCodePrdError::ExecutionFailed(format!(
                "Exit code {:?}: {}",
                output.status.code(),
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.trim().is_empty() {
            return Err(ClaudeCodePrdError::EmptyResponse);
        }

        info!("Claude Code CLI completed successfully, output length: {} bytes", stdout.len());
        Ok(stdout)
    }

    /// Parse the JSON output from Claude Code
    fn parse_output(output: &str) -> Result<AnthropicInboxResult, ClaudeCodePrdError> {
        // Claude Code with --output-format json returns a JSON object with a "result" field
        // containing the assistant's final text response
        let trimmed = output.trim();
        
        // Log first 500 chars of output for debugging
        debug!("Raw output (first 500 chars): {}", trimmed.chars().take(500).collect::<String>());

        // The zsh login shell might add extra text before the JSON
        // Find the first '{' which should be the start of the Claude Code JSON envelope
        let json_start = match trimmed.find("{\"type\":\"result\"") {
            Some(pos) => pos,
            None => trimmed.find('{').unwrap_or(0),
        };
        
        let json_output = &trimmed[json_start..];
        debug!("JSON output starts at position {}", json_start);

        // Try to parse as Claude Code JSON output format
        if let Ok(claude_output) = serde_json::from_str::<ClaudeCodeOutput>(json_output) {
            debug!("Parsed Claude Code output envelope successfully");
            // Extract the text content from the result
            if let Some(text) = claude_output.result {
                debug!("Result field (first 200 chars): {}", text.chars().take(200).collect::<String>());
                let json_text = strip_code_fences(&text);
                debug!("Stripped JSON (first 200 chars): {}", json_text.chars().take(200).collect::<String>());
                return serde_json::from_str(&json_text)
                    .map_err(|e| ClaudeCodePrdError::ParseError(format!("Failed to parse result JSON: {}", e)));
            } else {
                return Err(ClaudeCodePrdError::ParseError("Claude Code output has no result field".to_string()));
            }
        }

        // Try parsing each line as JSON (streaming format)
        for line in trimmed.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Try to parse as a streaming message
            if let Ok(msg) = serde_json::from_str::<StreamMessage>(line) {
                if msg.r#type == "result" {
                    if let Some(result) = msg.result {
                        let json_text = strip_code_fences(&result);
                        return serde_json::from_str(&json_text)
                            .map_err(|e| ClaudeCodePrdError::ParseError(e.to_string()));
                    }
                }
            }

            // Try direct JSON parse (the model might output just JSON)
            if line.starts_with('{') {
                let json_text = strip_code_fences(line);
                if let Ok(result) = serde_json::from_str::<AnthropicInboxResult>(&json_text) {
                    return Ok(result);
                }
            }
        }

        // Last resort: try to find any JSON object with the expected fields
        if let Some(start) = json_output.find('{') {
            if let Some(end) = json_output.rfind('}') {
                let json_text = &json_output[start..=end];
                if let Ok(result) = serde_json::from_str::<AnthropicInboxResult>(json_text) {
                    return Ok(result);
                }
            }
        }

        Err(ClaudeCodePrdError::ParseError(format!(
            "Could not find valid JSON in output: {}",
            if trimmed.len() > 200 {
                format!("{}...", &trimmed[..200])
            } else {
                trimmed.to_string()
            }
        )))
    }
}

/// Claude Code JSON output format
#[derive(Debug, Deserialize)]
struct ClaudeCodeOutput {
    result: Option<String>,
}

/// Claude Code streaming message format
#[derive(Debug, Deserialize)]
struct StreamMessage {
    r#type: String,
    result: Option<String>,
}

/// Strip markdown code fences from a string
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
    use db::models::inbox_item::InboxItemKind;

    #[test]
    fn test_strip_code_fences() {
        let raw = "```json\n{\"actionable\":true}\n```";
        let stripped = strip_code_fences(raw);
        assert_eq!(stripped, "{\"actionable\":true}");
    }

    #[test]
    fn test_strip_code_fences_no_fences() {
        let raw = "{\"actionable\":true}";
        let stripped = strip_code_fences(raw);
        assert_eq!(stripped, "{\"actionable\":true}");
    }

    #[test]
    fn test_parse_direct_json() {
        let json = r#"{"actionable":true,"kind":"feature","title":"Add export","prd_markdown":"Details","context_links":[]}"#;
        let result = ClaudeCodePrdService::parse_output(json).unwrap();
        assert!(result.actionable);
        assert!(matches!(result.kind, InboxItemKind::Feature));
        assert_eq!(result.title, "Add export");
    }

    #[test]
    fn test_build_prompt() {
        let prompt = ClaudeCodePrdService::build_prompt("Add dark mode", "## Problem\n...");
        assert!(prompt.contains("Add dark mode"));
        assert!(prompt.contains("## Problem"));
        assert!(prompt.contains("JSON schema"));
    }
}
