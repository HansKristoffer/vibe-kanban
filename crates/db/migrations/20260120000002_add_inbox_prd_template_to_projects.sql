-- Add per-project PRD template override for Inbox PRD generation
ALTER TABLE projects ADD COLUMN inbox_prd_template TEXT;
