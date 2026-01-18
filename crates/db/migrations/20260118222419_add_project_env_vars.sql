PRAGMA foreign_keys = ON;

-- Per-project environment variable values.
-- Variable names come from vibekanban.json allowlist; values are stored here.
CREATE TABLE project_env_vars (
    project_id  BLOB NOT NULL,
    name        TEXT NOT NULL,
    value       TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    PRIMARY KEY (project_id, name),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_project_env_vars_project_id ON project_env_vars(project_id);
