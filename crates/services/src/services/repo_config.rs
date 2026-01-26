//! Repository configuration file support.
//!
//! This module provides support for reading repository configuration from a
//! `vibekanban.json` file in the repository root. When present, these values
//! override the database-stored configuration.

use db::models::repo::{Repo, RepoWithEffectiveConfig};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use ts_rs::TS;

/// The name of the config file to look for in repository roots.
pub const CONFIG_FILE_NAME: &str = "vibekanban.json";

/// Root configuration structure for vibekanban.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VibeKanbanConfig {
    /// Scripts and related configuration.
    #[serde(default)]
    pub scripts: Option<ScriptsConfig>,

    /// Environment variable names to expose in the dev server.
    /// Values are stored per-project in the database.
    #[serde(default)]
    pub env_vars: Option<Vec<String>>,
}

/// Scripts configuration section.
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export)]
pub struct ScriptsConfig {
    /// Setup script to run before the coding agent starts.
    #[serde(default)]
    pub setup_script: Option<String>,

    /// Cleanup script to run after coding agent execution.
    #[serde(default)]
    pub cleanup_script: Option<String>,

    /// Dev server script for starting a development server.
    #[serde(default)]
    pub dev_server_script: Option<String>,

    /// Comma-separated list of files to copy to the worktree.
    #[serde(default)]
    pub copy_files: Option<String>,

    /// Whether to run the setup script in parallel with the coding agent.
    #[serde(default)]
    pub parallel_setup_script: Option<bool>,
}

/// Error type for config file operations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Reads the vibekanban.json config file from the given repository path.
///
/// Returns `None` if the file doesn't exist. Returns an error if the file
/// exists but cannot be read or parsed.
pub fn read_repo_config(repo_path: &Path) -> Result<Option<VibeKanbanConfig>, ConfigError> {
    let config_path = repo_path.join(CONFIG_FILE_NAME);

    if !config_path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&config_path)?;
    let config: VibeKanbanConfig = serde_json::from_str(&contents)?;

    tracing::debug!(
        "Loaded vibekanban.json config from {}",
        config_path.display()
    );

    Ok(Some(config))
}

/// Tries to read the vibekanban.json config file, returning None on any error.
///
/// This is a convenience function that logs errors but doesn't propagate them,
/// useful when you want to gracefully fall back to database values.
pub fn try_read_repo_config(repo_path: &Path) -> Option<VibeKanbanConfig> {
    match read_repo_config(repo_path) {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!(
                "Failed to read vibekanban.json from {}: {}",
                repo_path.display(),
                e
            );
            None
        }
    }
}

/// Checks if a vibekanban.json config file exists at the given repository path.
pub fn has_config_file(repo_path: &Path) -> bool {
    repo_path.join(CONFIG_FILE_NAME).exists()
}

/// Creates a RepoWithEffectiveConfig from a Repo by reading the config file.
///
/// If a vibekanban.json file exists, its values override the database values.
/// If the file doesn't exist or can't be read, returns the database values.
pub fn apply_config_to_repo(repo: Repo) -> RepoWithEffectiveConfig {
    let config = try_read_repo_config(&repo.path);

    match config {
        Some(VibeKanbanConfig {
            scripts: Some(scripts),
            ..
        }) => {
            // Config file exists with scripts section - apply overrides
            let setup_script_from_file = scripts.setup_script.is_some();
            let cleanup_script_from_file = scripts.cleanup_script.is_some();
            let dev_server_script_from_file = scripts.dev_server_script.is_some();
            let copy_files_from_file = scripts.copy_files.is_some();
            let parallel_setup_script_from_file = scripts.parallel_setup_script.is_some();

            RepoWithEffectiveConfig {
                id: repo.id,
                path: repo.path,
                name: repo.name,
                display_name: repo.display_name,
                has_config_file: true,
                setup_script: scripts.setup_script.or(repo.setup_script),
                setup_script_from_file,
                cleanup_script: scripts.cleanup_script.or(repo.cleanup_script),
                cleanup_script_from_file,
                dev_server_script: scripts.dev_server_script.or(repo.dev_server_script),
                dev_server_script_from_file,
                copy_files: scripts.copy_files.or(repo.copy_files),
                copy_files_from_file,
                parallel_setup_script: scripts
                    .parallel_setup_script
                    .unwrap_or(repo.parallel_setup_script),
                parallel_setup_script_from_file,
                created_at: repo.created_at,
                updated_at: repo.updated_at,
            }
        }
        Some(VibeKanbanConfig { scripts: None, .. }) => {
            // Config file exists but no scripts section
            RepoWithEffectiveConfig {
                id: repo.id,
                path: repo.path,
                name: repo.name,
                display_name: repo.display_name,
                has_config_file: true,
                setup_script: repo.setup_script,
                setup_script_from_file: false,
                cleanup_script: repo.cleanup_script,
                cleanup_script_from_file: false,
                dev_server_script: repo.dev_server_script,
                dev_server_script_from_file: false,
                copy_files: repo.copy_files,
                copy_files_from_file: false,
                parallel_setup_script: repo.parallel_setup_script,
                parallel_setup_script_from_file: false,
                created_at: repo.created_at,
                updated_at: repo.updated_at,
            }
        }
        None => {
            // No config file - use database values
            RepoWithEffectiveConfig::from_repo_without_config(repo)
        }
    }
}

/// Applies config file overrides to a list of repos.
pub fn apply_config_to_repos(repos: Vec<Repo>) -> Vec<RepoWithEffectiveConfig> {
    repos.into_iter().map(apply_config_to_repo).collect()
}

/// Creates an effective Repo (for script execution) by applying config file overrides.
///
/// This returns a modified Repo struct with config file values applied,
/// useful for places that need a Repo but want config file overrides.
pub fn get_effective_repo(repo: Repo) -> Repo {
    let config = try_read_repo_config(&repo.path);

    match config {
        Some(VibeKanbanConfig {
            scripts: Some(scripts),
            ..
        }) => Repo {
            setup_script: scripts.setup_script.or(repo.setup_script),
            cleanup_script: scripts.cleanup_script.or(repo.cleanup_script),
            dev_server_script: scripts.dev_server_script.or(repo.dev_server_script),
            copy_files: scripts.copy_files.or(repo.copy_files),
            parallel_setup_script: scripts
                .parallel_setup_script
                .unwrap_or(repo.parallel_setup_script),
            ..repo
        },
        _ => repo,
    }
}

/// Applies config file overrides to a list of repos, returning Repo structs.
pub fn get_effective_repos(repos: Vec<Repo>) -> Vec<Repo> {
    repos.into_iter().map(get_effective_repo).collect()
}

/// Validates an environment variable name.
/// Valid names match the pattern: starts with letter or underscore,
/// followed by letters, digits, or underscores.
fn is_valid_env_var_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Compile regex once per call; in hot paths consider lazy_static
    let re = Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").unwrap();
    re.is_match(trimmed)
}

/// Collects and deduplicates environment variable names from vibekanban.json
/// files across multiple repositories.
///
/// - Reads each repo's config file
/// - Filters to valid env var names (trimmed, matches `^[A-Za-z_][A-Za-z0-9_]*$`)
/// - Deduplicates while preserving first-seen order
pub fn collect_env_vars_for_repos(repos: &[Repo]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    tracing::debug!(
        "Collecting env vars from {} repositories",
        repos.len()
    );

    for repo in repos {
        let config_path = repo.path.join(CONFIG_FILE_NAME);
        tracing::debug!(
            "Checking repo '{}' at path: {} (config exists: {})",
            repo.name,
            config_path.display(),
            config_path.exists()
        );

        if let Some(config) = try_read_repo_config(&repo.path) {
            tracing::debug!(
                "Config file loaded for repo '{}', env_vars: {:?}",
                repo.name,
                config.env_vars
            );
            if let Some(env_vars) = config.env_vars {
                for name in env_vars {
                    let trimmed = name.trim().to_string();
                    if is_valid_env_var_name(&trimmed) && !seen.contains(&trimmed) {
                        seen.insert(trimmed.clone());
                        result.push(trimmed);
                    }
                }
            }
        } else {
            tracing::debug!(
                "No config file found for repo '{}' at {}",
                repo.name,
                config_path.display()
            );
        }
    }

    tracing::debug!("Collected {} env vars: {:?}", result.len(), result);
    result
}

/// Collects environment variable names from a single repository's config file.
pub fn collect_env_vars_for_repo(repo: &Repo) -> Vec<String> {
    collect_env_vars_for_repos(&[repo.clone()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_repo(path: PathBuf) -> Repo {
        Repo {
            id: Uuid::new_v4(),
            path,
            name: "test-repo".to_string(),
            display_name: "Test Repo".to_string(),
            setup_script: Some("db-setup".to_string()),
            cleanup_script: Some("db-cleanup".to_string()),
            dev_server_script: Some("db-dev".to_string()),
            copy_files: Some(".env".to_string()),
            parallel_setup_script: false,
            default_target_branch: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_read_missing_config() {
        let temp_dir = TempDir::new().unwrap();
        let result = read_repo_config(temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_valid_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "scripts": {
                "setup_script": "npm install",
                "dev_server_script": "npm run dev",
                "cleanup_script": "npm run lint",
                "copy_files": ".env",
                "parallel_setup_script": true
            }
        }"#;

        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let result = read_repo_config(temp_dir.path()).unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        let scripts = config.scripts.unwrap();
        assert_eq!(scripts.setup_script, Some("npm install".to_string()));
        assert_eq!(scripts.dev_server_script, Some("npm run dev".to_string()));
        assert_eq!(scripts.cleanup_script, Some("npm run lint".to_string()));
        assert_eq!(scripts.copy_files, Some(".env".to_string()));
        assert_eq!(scripts.parallel_setup_script, Some(true));
    }

    #[test]
    fn test_read_partial_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "scripts": {
                "setup_script": "bun install"
            }
        }"#;

        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let result = read_repo_config(temp_dir.path()).unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        let scripts = config.scripts.unwrap();
        assert_eq!(scripts.setup_script, Some("bun install".to_string()));
        assert!(scripts.dev_server_script.is_none());
        assert!(scripts.cleanup_script.is_none());
    }

    #[test]
    fn test_read_empty_config() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), "{}").unwrap();

        let result = read_repo_config(temp_dir.path()).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().scripts.is_none());
    }

    #[test]
    fn test_read_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), "not valid json").unwrap();

        let result = read_repo_config(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_has_config_file() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!has_config_file(temp_dir.path()));

        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), "{}").unwrap();
        assert!(has_config_file(temp_dir.path()));
    }

    #[test]
    fn test_apply_config_to_repo_no_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let repo = create_test_repo(temp_dir.path().to_path_buf());

        let effective = apply_config_to_repo(repo);

        assert!(!effective.has_config_file);
        assert_eq!(effective.setup_script, Some("db-setup".to_string()));
        assert!(!effective.setup_script_from_file);
        assert_eq!(effective.cleanup_script, Some("db-cleanup".to_string()));
        assert!(!effective.cleanup_script_from_file);
    }

    #[test]
    fn test_apply_config_to_repo_with_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "scripts": {
                "setup_script": "file-setup",
                "parallel_setup_script": true
            }
        }"#;
        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let repo = create_test_repo(temp_dir.path().to_path_buf());
        let effective = apply_config_to_repo(repo);

        assert!(effective.has_config_file);
        // setup_script should come from file
        assert_eq!(effective.setup_script, Some("file-setup".to_string()));
        assert!(effective.setup_script_from_file);
        // cleanup_script should come from database (not in config file)
        assert_eq!(effective.cleanup_script, Some("db-cleanup".to_string()));
        assert!(!effective.cleanup_script_from_file);
        // parallel_setup_script should come from file
        assert!(effective.parallel_setup_script);
        assert!(effective.parallel_setup_script_from_file);
    }

    #[test]
    fn test_get_effective_repo() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "scripts": {
                "setup_script": "file-setup"
            }
        }"#;
        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let repo = create_test_repo(temp_dir.path().to_path_buf());
        let effective = get_effective_repo(repo);

        // setup_script should come from file
        assert_eq!(effective.setup_script, Some("file-setup".to_string()));
        // cleanup_script should come from database
        assert_eq!(effective.cleanup_script, Some("db-cleanup".to_string()));
    }

    #[test]
    fn test_read_config_with_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "env_vars": ["API_KEY", "DATABASE_URL", "SECRET_TOKEN"]
        }"#;

        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let result = read_repo_config(temp_dir.path()).unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        assert!(config.env_vars.is_some());
        let env_vars = config.env_vars.unwrap();
        assert_eq!(env_vars.len(), 3);
        assert_eq!(env_vars[0], "API_KEY");
        assert_eq!(env_vars[1], "DATABASE_URL");
        assert_eq!(env_vars[2], "SECRET_TOKEN");
    }

    #[test]
    fn test_collect_env_vars_for_repos_valid_names() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "env_vars": ["VALID_NAME", "_also_valid", "Another123"]
        }"#;
        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let repo = create_test_repo(temp_dir.path().to_path_buf());
        let env_vars = collect_env_vars_for_repos(&[repo]);

        assert_eq!(env_vars.len(), 3);
        assert!(env_vars.contains(&"VALID_NAME".to_string()));
        assert!(env_vars.contains(&"_also_valid".to_string()));
        assert!(env_vars.contains(&"Another123".to_string()));
    }

    #[test]
    fn test_collect_env_vars_filters_invalid_names() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "env_vars": ["VALID", "123invalid", "has-dash", "has space", "", "ok_name"]
        }"#;
        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let repo = create_test_repo(temp_dir.path().to_path_buf());
        let env_vars = collect_env_vars_for_repos(&[repo]);

        // Only VALID and ok_name should pass validation
        assert_eq!(env_vars.len(), 2);
        assert!(env_vars.contains(&"VALID".to_string()));
        assert!(env_vars.contains(&"ok_name".to_string()));
    }

    #[test]
    fn test_collect_env_vars_deduplicates() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();

        let config1 = r#"{ "env_vars": ["API_KEY", "DATABASE_URL"] }"#;
        let config2 = r#"{ "env_vars": ["DATABASE_URL", "SECRET"] }"#;

        fs::write(temp_dir1.path().join(CONFIG_FILE_NAME), config1).unwrap();
        fs::write(temp_dir2.path().join(CONFIG_FILE_NAME), config2).unwrap();

        let repo1 = create_test_repo(temp_dir1.path().to_path_buf());
        let mut repo2 = create_test_repo(temp_dir2.path().to_path_buf());
        repo2.name = "test-repo-2".to_string();

        let env_vars = collect_env_vars_for_repos(&[repo1, repo2]);

        // DATABASE_URL appears in both but should only appear once
        assert_eq!(env_vars.len(), 3);
        assert_eq!(env_vars[0], "API_KEY");
        assert_eq!(env_vars[1], "DATABASE_URL");
        assert_eq!(env_vars[2], "SECRET");
    }

    #[test]
    fn test_collect_env_vars_trims_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"{
            "env_vars": ["  PADDED  ", "NORMAL"]
        }"#;
        fs::write(temp_dir.path().join(CONFIG_FILE_NAME), config_content).unwrap();

        let repo = create_test_repo(temp_dir.path().to_path_buf());
        let env_vars = collect_env_vars_for_repos(&[repo]);

        assert_eq!(env_vars.len(), 2);
        assert!(env_vars.contains(&"PADDED".to_string()));
        assert!(env_vars.contains(&"NORMAL".to_string()));
    }

    #[test]
    fn test_collect_env_vars_no_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let repo = create_test_repo(temp_dir.path().to_path_buf());
        let env_vars = collect_env_vars_for_repos(&[repo]);

        assert!(env_vars.is_empty());
    }

    #[test]
    fn test_is_valid_env_var_name() {
        assert!(is_valid_env_var_name("VALID"));
        assert!(is_valid_env_var_name("_underscore"));
        assert!(is_valid_env_var_name("Mix123"));
        assert!(is_valid_env_var_name("a"));

        assert!(!is_valid_env_var_name("123start"));
        assert!(!is_valid_env_var_name("has-dash"));
        assert!(!is_valid_env_var_name("has space"));
        assert!(!is_valid_env_var_name(""));
        assert!(!is_valid_env_var_name("  "));
    }
}
