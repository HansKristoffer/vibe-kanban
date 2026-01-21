-- Add column to track which Slack user created the inbox item via /vibe command
ALTER TABLE inbox_items ADD COLUMN slack_created_by_user_id TEXT;
