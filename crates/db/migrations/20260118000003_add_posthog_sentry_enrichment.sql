ALTER TABLE project_integrations ADD COLUMN posthog_api_key TEXT;
ALTER TABLE project_integrations ADD COLUMN posthog_host TEXT;
ALTER TABLE project_integrations ADD COLUMN posthog_project_id TEXT;
ALTER TABLE project_integrations ADD COLUMN sentry_api_token TEXT;
ALTER TABLE project_integrations ADD COLUMN sentry_org_slug TEXT;
ALTER TABLE project_integrations ADD COLUMN sentry_project_slug TEXT;
