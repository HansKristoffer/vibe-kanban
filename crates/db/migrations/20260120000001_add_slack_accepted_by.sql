-- Add column to track which Slack user accepted the PRD
ALTER TABLE inbox_items ADD COLUMN slack_accepted_by_user_id TEXT;
