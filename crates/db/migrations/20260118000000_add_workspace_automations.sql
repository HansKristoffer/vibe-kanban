PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS workspace_automations (
    workspace_id        BLOB PRIMARY KEY,
    mode                TEXT NOT NULL
                        CHECK (mode IN ('ralph')),
    status              TEXT NOT NULL
                        CHECK (status IN ('running','paused','stopped','completed')),
    iteration           INTEGER NOT NULL DEFAULT 0,
    max_iterations      INTEGER NOT NULL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    max_failures        INTEGER NOT NULL,
    last_error          TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_workspace_automations_status
    ON workspace_automations (status);
