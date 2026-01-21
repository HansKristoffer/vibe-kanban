-- Add column to store the Slack thread reply timestamp for the PRD content
-- This allows updating the PRD in the thread instead of posting new replies
ALTER TABLE inbox_items ADD COLUMN slack_prd_thread_ts TEXT;
