PRAGMA foreign_keys = ON;

CREATE TABLE project_integrations (
    project_id                BLOB PRIMARY KEY,
    webhook_token             TEXT NOT NULL UNIQUE,
    linear_api_key            TEXT,
    linear_team_id            TEXT,
    linear_state_id_todo      TEXT,
    linear_state_id_inprogress TEXT,
    linear_state_id_inreview  TEXT,
    linear_state_id_done      TEXT,
    linear_state_id_cancelled TEXT,
    linear_webhook_secret     TEXT,
    intercom_access_token     TEXT,
    intercom_webhook_secret   TEXT,
    intercom_admin_id         TEXT,
    modjo_api_key             TEXT,
    modjo_webhook_secret      TEXT,
    created_at                TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at                TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE inbox_items (
    id                      BLOB PRIMARY KEY,
    project_id              BLOB NOT NULL,
    source                  TEXT NOT NULL
                             CHECK (source IN ('linear','intercom','modjo','manual')),
    source_item_id          TEXT NOT NULL,
    source_url              TEXT,
    title                   TEXT NOT NULL,
    raw_payload_json        TEXT,
    kind                    TEXT NOT NULL DEFAULT 'other'
                             CHECK (kind IN ('bug','feature','other')),
    status                  TEXT NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending','accepted','declined','ignored')),
    prd_markdown            TEXT,
    task_id                 BLOB,
    linear_issue_id         TEXT,
    linear_issue_url        TEXT,
    action_token            TEXT NOT NULL UNIQUE,
    outbound_registered_at  TEXT,
    outbound_started_at     TEXT,
    outbound_pr_created_at  TEXT,
    outbound_pr_merged_at   TEXT,
    outbound_last_error     TEXT,
    created_at              TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL,
    UNIQUE (project_id, source, source_item_id)
);

CREATE INDEX idx_inbox_items_project_id ON inbox_items(project_id);
CREATE INDEX idx_inbox_items_status ON inbox_items(status);
CREATE INDEX idx_inbox_items_source ON inbox_items(source);
CREATE INDEX idx_inbox_items_task_id ON inbox_items(task_id);

CREATE TABLE inbox_source_cursors (
    project_id  BLOB NOT NULL,
    source      TEXT NOT NULL
                 CHECK (source IN ('linear','intercom','modjo','manual')),
    cursor      TEXT,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    PRIMARY KEY (project_id, source),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
