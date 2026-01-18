Repository Config File (vibekanban.json)

Overview
- Allows defining repository scripts and configuration via a `vibekanban.json` file in the repo root.
- Config file values completely override database-stored settings.
- Config file is read fresh each time a workspace is created/started.
- UI shows config-file-sourced values as read-only with visual indicators.

Config file format
- File must be named `vibekanban.json` and placed in the repository root.
- All fields are optional; only specified fields override database values.

```json
{
  "scripts": {
    "setup_script": "npm install",
    "dev_server_script": "npm run dev",
    "cleanup_script": "npm run lint && npm run format",
    "copy_files": ".env, config/*.json",
    "parallel_setup_script": false
  },
  "env_vars": ["API_KEY", "DATABASE_URL"]
}
```

Available script fields:
- `setup_script`: Runs before the coding agent starts (e.g., install dependencies).
- `cleanup_script`: Runs after coding agent execution if changes were made.
- `dev_server_script`: Command to start the development server.
- `copy_files`: Comma-separated files/globs to copy from original repo to worktree.
- `parallel_setup_script`: Boolean; run setup in parallel with coding agent.

Environment variables:
- `env_vars`: Array of environment variable names to expose in the dev server.
  - Values are configured per-project in Project Settings.
  - Only variables listed in the config file will be available.
  - Variable names must match `^[A-Za-z_][A-Za-z0-9_]*$` (invalid names are ignored).
  - Variables from multiple repos in a project are merged (first-seen order, deduplicated).

Backend implementation

Config reading:
- Module: crates/services/src/services/repo_config.rs
- Key types:
  - `VibeKanbanConfig`: Root config structure
  - `ScriptsConfig`: Scripts section with all script fields
- Key functions:
  - `read_repo_config(path)`: Reads and parses vibekanban.json, returns None if missing.
  - `try_read_repo_config(path)`: Same but logs errors and returns None on failure.
  - `apply_config_to_repo(repo)`: Creates RepoWithEffectiveConfig with overrides applied.
  - `get_effective_repo(repo)`: Returns Repo with config values applied (for execution).

Config application:
- Applied in crates/services/src/services/container.rs at script execution time.
- Functions that apply config:
  - `cleanup_actions_for_repos()`: Builds cleanup script actions with overrides.
  - `setup_actions_for_repos()`: Builds setup script actions with overrides.
  - `setup_action_for_repo()`: Single repo setup action with overrides.
  - `build_sequential_setup_chain()`: Chains setup scripts with overrides.

Dev server environment injection:
- In crates/local-deployment/src/container.rs `start_execution_inner()`.
- When `ExecutionProcessRunReason::DevServer`:
  - Collects allowed env var names from repos' vibekanban.json files.
  - Loads configured values from `project_env_vars` database table.
  - Injects matching values into the execution environment.

Data model:
- `RepoWithEffectiveConfig` in crates/db/src/models/repo.rs:
  - Contains all Repo fields plus:
  - `has_config_file: bool`: Whether vibekanban.json exists.
  - `*_from_file: bool`: For each script field, whether value comes from file.

- `ProjectEnvVar` in crates/db/src/models/project_env_var.rs:
  - Stores per-project environment variable values.
  - Fields: `project_id`, `name`, `value`, `created_at`, `updated_at`.
  - Composite primary key: `(project_id, name)`.
  - Values stored in plaintext (similar to other integration secrets).

Database table:
- Table: `project_env_vars`
- Migration: crates/db/migrations/20260118222419_add_project_env_vars.sql

API endpoints

Get effective config:
- `GET /api/repos/{repo_id}/effective-config`
- Returns: `RepoWithEffectiveConfig` with config file values applied and source indicators.
- Handler: `get_repo_effective_config()` in crates/server/src/routes/repo.rs

Project environment variables:
- `GET /api/projects/{project_id}/env-vars`
  - Returns list of env var names from all project repos' vibekanban.json files.
  - Response includes `{ name, configured }` for each variable (values are not exposed).
- `PUT /api/projects/{project_id}/env-vars`
  - Body: `{ set: Record<string, string>, clear: string[] }`
  - Only accepts names from the config file allowlist.
  - Handler: crates/server/src/routes/project_env_vars.rs

Frontend implementation

API client:
- `repoApi.getEffectiveConfig(repoId)` in frontend/src/lib/api.ts
- Returns: `RepoWithEffectiveConfig`
- `projectEnvVarsApi` in frontend/src/lib/api.ts:
  - `get(projectId)`: Fetches env var list with configured status.
  - `update(projectId, { set, clear })`: Sets or clears env var values.

Repository Settings UI:
- File: frontend/src/pages/settings/ReposSettings.tsx
- Fetches effective config when repo is selected.
- Shows alert banner when config file is detected.
- Displays "vibekanban.json" badge on fields sourced from config file.
- Config-sourced fields are disabled/read-only with visual styling.

Project Settings UI (Environment Variables):
- File: frontend/src/pages/settings/ProjectSettings.tsx
- "Dev Server Environment Variables" card section.
- Lists all env var names from project repos' vibekanban.json files.
- Password inputs for each variable (values are masked).
- Per-variable Save button to set a value.
- Per-variable Clear button (trash icon) shown when value is configured.
- Placeholder shows "Configured" when a value exists, "Not set" otherwise.

i18n keys (settings.repos.configFile):
- `badge`: Label for the config file badge.
- `notice`: Alert message when config file is detected.
- `fieldReadOnly`: Helper text for read-only fields.

Behavior notes
- Config file is read on every workspace start (not cached).
- If config file has parse errors, they are logged and database values are used.
- Empty or missing `scripts` section means no overrides (database values used).
- Config file presence is detected even if it has no scripts section.
- Database values can still be set via UI for fields not in config file.

Usage example: Environment variables for dev server

1. Add `env_vars` to your vibekanban.json:
```json
{
  "scripts": {
    "dev_server_script": "npm run dev"
  },
  "env_vars": ["INFICICAL_KEY", "API_SECRET"]
}
```

2. Go to Settings > Projects and select your project.

3. Scroll to "Dev Server Environment Variables" section.
   - You'll see input fields for each variable defined in env_vars.

4. Enter values for each variable and click Save.
   - Values are stored securely per-project.
   - The actual values are never returned by the API (only "configured" status).

5. Start the dev server via Preview.
   - The configured environment variables are automatically injected.
   - Only variables in the vibekanban.json allowlist are included.

Security notes:
- Env var values are stored in the local SQLite database (plaintext).
- API responses only indicate whether a value is configured, never the actual value.
- UI uses password inputs to mask values during entry.
- Variable names act as an allowlist; arbitrary env vars cannot be injected.

Testing
- Unit tests in crates/services/src/services/repo_config.rs cover:
  - Missing config file
  - Valid config with all fields
  - Partial config (some fields only)
  - Empty config
  - Invalid JSON
  - Config application to repos
  - Env vars parsing and validation
  - Env vars deduplication across repos
  - Invalid env var name filtering
