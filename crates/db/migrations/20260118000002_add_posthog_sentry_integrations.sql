PRAGMA foreign_keys = OFF;

ALTER TABLE project_integrations ADD COLUMN posthog_webhook_secret TEXT;
ALTER TABLE project_integrations ADD COLUMN sentry_webhook_secret TEXT;

ALTER TABLE inbox_items RENAME TO inbox_items_old;

CREATE TABLE inbox_items (
    id                      BLOB PRIMARY KEY,
    project_id              BLOB NOT NULL,
    source                  TEXT NOT NULL
                             CHECK (source IN ('linear','intercom','modjo','manual','posthog','sentry')),
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

INSERT INTO inbox_items (
    id,
    project_id,
    source,
    source_item_id,
    source_url,
    title,
    raw_payload_json,
    kind,
    status,
    prd_markdown,
    task_id,
    linear_issue_id,
    linear_issue_url,
    action_token,
    outbound_registered_at,
    outbound_started_at,
    outbound_pr_created_at,
    outbound_pr_merged_at,
    outbound_last_error,
    created_at,
    updated_at
)
SELECT
    id,
    project_id,
    source,
    source_item_id,
    source_url,
    title,
    raw_payload_json,
    kind,
    status,
    prd_markdown,
    task_id,
    linear_issue_id,
    linear_issue_url,
    action_token,
    outbound_registered_at,
    outbound_started_at,
    outbound_pr_created_at,
    outbound_pr_merged_at,
    outbound_last_error,
    created_at,
    updated_at
FROM inbox_items_old;

DROP TABLE inbox_items_old;

CREATE INDEX idx_inbox_items_project_id ON inbox_items(project_id);
CREATE INDEX idx_inbox_items_status ON inbox_items(status);
CREATE INDEX idx_inbox_items_source ON inbox_items(source);
CREATE INDEX idx_inbox_items_task_id ON inbox_items(task_id);

ALTER TABLE inbox_source_cursors RENAME TO inbox_source_cursors_old;

CREATE TABLE inbox_source_cursors (
    project_id  BLOB NOT NULL,
    source      TEXT NOT NULL
                 CHECK (source IN ('linear','intercom','modjo','manual','posthog','sentry')),
    cursor      TEXT,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    PRIMARY KEY (project_id, source),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

INSERT INTO inbox_source_cursors (
    project_id,
    source,
    cursor,
    updated_at
)
SELECT
    project_id,
    source,
    cursor,
    updated_at
FROM inbox_source_cursors_old;

DROP TABLE inbox_source_cursors_old;

PRAGMA foreign_keys = ON;
