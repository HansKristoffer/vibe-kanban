-- Add prd_generating flag to track when PRD generation is in progress
ALTER TABLE inbox_items ADD COLUMN prd_generating INTEGER NOT NULL DEFAULT 0;
