-- Add Slack integration columns to project_integrations
ALTER TABLE project_integrations ADD COLUMN slack_bot_token TEXT;
ALTER TABLE project_integrations ADD COLUMN slack_signing_secret TEXT;
ALTER TABLE project_integrations ADD COLUMN slack_channel_id TEXT;

-- Add Slack message linkage columns to inbox_items
-- We need to recreate the table to add the new source type 'slack' and columns
PRAGMA foreign_keys = OFF;

ALTER TABLE inbox_items RENAME TO inbox_items_old;

CREATE TABLE inbox_items (
    id                      BLOB PRIMARY KEY,
    project_id              BLOB NOT NULL,
    source                  TEXT NOT NULL
                             CHECK (source IN ('linear','intercom','modjo','manual','posthog','sentry','slack')),
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
    slack_channel_id        TEXT,
    slack_message_ts        TEXT,
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
    slack_channel_id,
    slack_message_ts,
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
    NULL,
    NULL,
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
CREATE INDEX idx_inbox_items_slack_message_ts ON inbox_items(slack_message_ts);

PRAGMA foreign_keys = ON;
