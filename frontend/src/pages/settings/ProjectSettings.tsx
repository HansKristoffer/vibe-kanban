import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { isEqual } from 'lodash';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { ChevronDown, Loader2, Plus, Trash2 } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { Checkbox } from '@/components/ui/checkbox';
import { useProjects } from '@/hooks/useProjects';
import { useProjectMutations } from '@/hooks/useProjectMutations';
import { RepoPickerDialog } from '@/components/dialogs/shared/RepoPickerDialog';
import { projectIntegrationsApi, projectsApi, projectEnvVarsApi, type EnvVarEntry } from '@/lib/api';
import { repoBranchKeys } from '@/hooks/useRepoBranches';
import type {
  LinearTeam,
  LinearWorkflowState,
  Project,
  ProjectIntegrationsResponse,
  Repo,
  UpdateProject,
  UpdateProjectIntegrationsRequest,
} from 'shared/types';
import { DEFAULT_INBOX_PRD_TEMPLATE } from 'shared/types';

interface ProjectFormState {
  name: string;
  inbox_prd_template: string | null;
}

function projectToFormState(project: Project): ProjectFormState {
  return {
    name: project.name,
    inbox_prd_template: project.inbox_prd_template ?? null,
  };
}

export function ProjectSettings() {
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const projectIdParam = searchParams.get('projectId') ?? '';
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();

  // Fetch all projects
  const {
    projects,
    isLoading: projectsLoading,
    error: projectsError,
  } = useProjects();

  // Selected project state
  const [selectedProjectId, setSelectedProjectId] = useState<string>(
    searchParams.get('projectId') || ''
  );
  const [selectedProject, setSelectedProject] = useState<Project | null>(null);

  // Form state
  const [draft, setDraft] = useState<ProjectFormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Repositories state
  const [repositories, setRepositories] = useState<Repo[]>([]);
  const [loadingRepos, setLoadingRepos] = useState(false);
  const [repoError, setRepoError] = useState<string | null>(null);
  const [addingRepo, setAddingRepo] = useState(false);
  const [deletingRepoId, setDeletingRepoId] = useState<string | null>(null);

  // Integrations state
  const [integrations, setIntegrations] =
    useState<ProjectIntegrationsResponse | null>(null);
  const [integrationsLoading, setIntegrationsLoading] = useState(false);
  const [integrationsError, setIntegrationsError] = useState<string | null>(null);
  const [savingIntegrations, setSavingIntegrations] = useState(false);

  const [linearApiKey, setLinearApiKey] = useState('');
  const [linearTeamId, setLinearTeamId] = useState('');
  const [linearStateTodo, setLinearStateTodo] = useState('');
  const [linearStateInProgress, setLinearStateInProgress] = useState('');
  const [linearStateInReview, setLinearStateInReview] = useState('');
  const [linearStateDone, setLinearStateDone] = useState('');
  const [linearStateCancelled, setLinearStateCancelled] = useState('');
  const [linearWebhookSecret, setLinearWebhookSecret] = useState('');

  const [intercomAccessToken, setIntercomAccessToken] = useState('');
  const [intercomAdminId, setIntercomAdminId] = useState('');
  const [intercomWebhookSecret, setIntercomWebhookSecret] = useState('');

  const [modjoApiKey, setModjoApiKey] = useState('');
  const [modjoWebhookSecret, setModjoWebhookSecret] = useState('');
  const [posthogWebhookSecret, setPosthogWebhookSecret] = useState('');
  const [sentryWebhookSecret, setSentryWebhookSecret] = useState('');
  const [posthogApiKey, setPosthogApiKey] = useState('');
  const [posthogHost, setPosthogHost] = useState('');
  const [posthogProjectId, setPosthogProjectId] = useState('');
  const [sentryApiToken, setSentryApiToken] = useState('');
  const [sentryOrgSlug, setSentryOrgSlug] = useState('');
  const [sentryProjectSlug, setSentryProjectSlug] = useState('');

  // Slack integration state
  const [slackBotToken, setSlackBotToken] = useState('');
  const [slackSigningSecret, setSlackSigningSecret] = useState('');
  const [slackChannelId, setSlackChannelId] = useState('');

  // Integration section enabled states (for collapsible UI)
  const [linearEnabled, setLinearEnabled] = useState(false);
  const [intercomEnabled, setIntercomEnabled] = useState(false);
  const [modjoEnabled, setModjoEnabled] = useState(false);
  const [posthogEnabled, setPosthogEnabled] = useState(false);
  const [sentryEnabled, setSentryEnabled] = useState(false);
  const [slackEnabled, setSlackEnabled] = useState(false);

  // Environment variables state
  const [envVars, setEnvVars] = useState<EnvVarEntry[]>([]);
  const [envVarsLoading, setEnvVarsLoading] = useState(false);
  const [envVarsError, setEnvVarsError] = useState<string | null>(null);
  const [envVarInputs, setEnvVarInputs] = useState<Record<string, string>>({});
  const [savingEnvVar, setSavingEnvVar] = useState<string | null>(null);

  // Check for unsaved changes (project name)
  const hasUnsavedChanges = useMemo(() => {
    if (!draft || !selectedProject) return false;
    return !isEqual(draft, projectToFormState(selectedProject));
  }, [draft, selectedProject]);

  const webhookUrls = useMemo(() => {
    if (!integrations) return null;
    if (integrations.webhook_urls) return integrations.webhook_urls;

    const base = window.location.origin.replace(/\/$/, '');
    const token = integrations.webhook_token;

    return {
      linear: `${base}/api/webhooks/linear/${token}`,
      intercom: `${base}/api/webhooks/intercom/${token}`,
      modjo: `${base}/api/webhooks/modjo/${token}`,
      manual: `${base}/api/webhooks/manual/${token}`,
      posthog: `${base}/api/webhooks/posthog/${token}`,
      sentry: `${base}/api/webhooks/sentry/${token}`,
      slack_commands: `${base}/api/webhooks/slack/commands`,
      slack_interactivity: `${base}/api/webhooks/slack/interactivity`,
    };
  }, [integrations]);

  // Handle project selection from dropdown
  const handleProjectSelect = useCallback(
    (id: string) => {
      // No-op if same project
      if (id === selectedProjectId) return;

      // Confirm if there are unsaved changes
      if (hasUnsavedChanges) {
        const confirmed = window.confirm(
          t('settings.projects.save.confirmSwitch')
        );
        if (!confirmed) return;

        // Clear local state before switching
        setDraft(null);
        setSelectedProject(null);
        setSuccess(false);
        setError(null);
      }

      // Update state and URL
      setSelectedProjectId(id);
      if (id) {
        setSearchParams({ projectId: id });
      } else {
        setSearchParams({});
      }
    },
    [hasUnsavedChanges, selectedProjectId, setSearchParams, t]
  );

  // Sync selectedProjectId when URL changes (with unsaved changes prompt)
  useEffect(() => {
    if (projectIdParam === selectedProjectId) return;

    // Confirm if there are unsaved changes
    if (hasUnsavedChanges) {
      const confirmed = window.confirm(
        t('settings.projects.save.confirmSwitch')
      );
      if (!confirmed) {
        // Revert URL to previous value
        if (selectedProjectId) {
          setSearchParams({ projectId: selectedProjectId });
        } else {
          setSearchParams({});
        }
        return;
      }

      // Clear local state before switching
      setDraft(null);
      setSelectedProject(null);
      setSuccess(false);
      setError(null);
    }

    setSelectedProjectId(projectIdParam);
  }, [
    projectIdParam,
    hasUnsavedChanges,
    selectedProjectId,
    setSearchParams,
    t,
  ]);

  // Populate draft from server data
  useEffect(() => {
    if (!projects) return;

    const nextProject = selectedProjectId
      ? projects.find((p) => p.id === selectedProjectId)
      : null;

    setSelectedProject((prev) =>
      prev?.id === nextProject?.id ? prev : (nextProject ?? null)
    );

    if (!nextProject) {
      if (!hasUnsavedChanges) setDraft(null);
      return;
    }

    if (hasUnsavedChanges) return;

    setDraft(projectToFormState(nextProject));
  }, [projects, selectedProjectId, hasUnsavedChanges]);

  // Warn on tab close/navigation with unsaved changes
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (hasUnsavedChanges) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [hasUnsavedChanges]);

  // Fetch repositories when project changes
  useEffect(() => {
    if (!selectedProjectId) {
      setRepositories([]);
      return;
    }

    setLoadingRepos(true);
    setRepoError(null);
    projectsApi
      .getRepositories(selectedProjectId)
      .then(setRepositories)
      .catch((err) => {
        setRepoError(
          err instanceof Error ? err.message : 'Failed to load repositories'
        );
        setRepositories([]);
      })
      .finally(() => setLoadingRepos(false));
  }, [selectedProjectId]);

  useEffect(() => {
    if (!selectedProjectId) {
      setIntegrations(null);
      return;
    }

    setIntegrationsLoading(true);
    setIntegrationsError(null);
    projectIntegrationsApi
      .get(selectedProjectId)
      .then((data) => {
        setIntegrations(data);
        setLinearTeamId(data.linear_team_id ?? '');
        setLinearStateTodo(data.linear_state_id_todo ?? '');
        setLinearStateInProgress(data.linear_state_id_inprogress ?? '');
        setLinearStateInReview(data.linear_state_id_inreview ?? '');
        setLinearStateDone(data.linear_state_id_done ?? '');
        setLinearStateCancelled(data.linear_state_id_cancelled ?? '');
        setPosthogHost(data.posthog_host ?? '');
        setPosthogProjectId(data.posthog_project_id ?? '');
        setSentryOrgSlug(data.sentry_org_slug ?? '');
        setSentryProjectSlug(data.sentry_project_slug ?? '');
        setSlackChannelId(data.slack_channel_id ?? '');

        // Set enabled states based on whether any config exists
        setLinearEnabled(
          data.linear_api_key_configured ||
          data.linear_webhook_secret_configured ||
          Boolean(data.linear_team_id)
        );
        setIntercomEnabled(
          data.intercom_access_token_configured ||
          data.intercom_webhook_secret_configured ||
          Boolean(data.intercom_admin_id)
        );
        setModjoEnabled(
          data.modjo_api_key_configured ||
          data.modjo_webhook_secret_configured
        );
        setPosthogEnabled(
          data.posthog_api_key_configured ||
          data.posthog_webhook_secret_configured ||
          Boolean(data.posthog_host) ||
          Boolean(data.posthog_project_id)
        );
        setSentryEnabled(
          data.sentry_api_token_configured ||
          data.sentry_webhook_secret_configured ||
          Boolean(data.sentry_org_slug) ||
          Boolean(data.sentry_project_slug)
        );
        setSlackEnabled(
          data.slack_bot_token_configured ||
          data.slack_signing_secret_configured ||
          Boolean(data.slack_channel_id)
        );
      })
      .catch((err) => {
        setIntegrationsError(
          err instanceof Error ? err.message : 'Failed to load integrations'
        );
        setIntegrations(null);
      })
      .finally(() => setIntegrationsLoading(false));
  }, [selectedProjectId]);

  // Fetch environment variables when project changes
  useEffect(() => {
    if (!selectedProjectId) {
      setEnvVars([]);
      setEnvVarInputs({});
      return;
    }

    setEnvVarsLoading(true);
    setEnvVarsError(null);
    projectEnvVarsApi
      .get(selectedProjectId)
      .then((data) => {
        setEnvVars(data.env_vars);
        // Clear inputs when loading new data
        setEnvVarInputs({});
      })
      .catch((err) => {
        setEnvVarsError(
          err instanceof Error ? err.message : 'Failed to load environment variables'
        );
        setEnvVars([]);
      })
      .finally(() => setEnvVarsLoading(false));
  }, [selectedProjectId]);

  const handleSaveEnvVar = async (name: string) => {
    if (!selectedProjectId) return;
    const value = envVarInputs[name];
    if (!value?.trim()) return;

    setSavingEnvVar(name);
    setEnvVarsError(null);
    try {
      const result = await projectEnvVarsApi.update(selectedProjectId, {
        set: { [name]: value.trim() },
      });
      setEnvVars(result.env_vars);
      setEnvVarInputs((prev) => ({ ...prev, [name]: '' }));
    } catch (err) {
      setEnvVarsError(
        err instanceof Error ? err.message : 'Failed to save environment variable'
      );
    } finally {
      setSavingEnvVar(null);
    }
  };

  const handleClearEnvVar = async (name: string) => {
    if (!selectedProjectId) return;

    setSavingEnvVar(name);
    setEnvVarsError(null);
    try {
      const result = await projectEnvVarsApi.update(selectedProjectId, {
        clear: [name],
      });
      setEnvVars(result.env_vars);
      setEnvVarInputs((prev) => ({ ...prev, [name]: '' }));
    } catch (err) {
      setEnvVarsError(
        err instanceof Error ? err.message : 'Failed to clear environment variable'
      );
    } finally {
      setSavingEnvVar(null);
    }
  };

  const linearTeamsQuery = useQuery<LinearTeam[]>({
    queryKey: ['linear-teams', selectedProjectId],
    enabled:
      Boolean(selectedProjectId) &&
      Boolean(integrations?.linear_api_key_configured),
    queryFn: () => projectIntegrationsApi.getLinearTeams(selectedProjectId),
  });

  const linearStatesQuery = useQuery<LinearWorkflowState[]>({
    queryKey: ['linear-states', selectedProjectId, linearTeamId],
    enabled:
      Boolean(selectedProjectId) &&
      Boolean(linearTeamId) &&
      Boolean(integrations?.linear_api_key_configured),
    queryFn: () => projectIntegrationsApi.getLinearStates(selectedProjectId, linearTeamId),
  });

  const handleAddRepository = async () => {
    if (!selectedProjectId) return;

    const repo = await RepoPickerDialog.show({
      title: 'Select Git Repository',
      description: 'Choose a git repository to add to this project',
    });

    if (!repo) return;

    if (repositories.some((r) => r.id === repo.id)) {
      return;
    }

    setAddingRepo(true);
    setRepoError(null);
    try {
      const newRepo = await projectsApi.addRepository(selectedProjectId, {
        display_name: repo.display_name,
        git_repo_path: repo.path,
      });
      setRepositories((prev) => [...prev, newRepo]);
      queryClient.invalidateQueries({
        queryKey: ['projectRepositories', selectedProjectId],
      });
      queryClient.invalidateQueries({
        queryKey: ['repos'],
      });
      queryClient.invalidateQueries({
        queryKey: repoBranchKeys.byRepo(newRepo.id),
      });
    } catch (err) {
      setRepoError(
        err instanceof Error ? err.message : 'Failed to add repository'
      );
    } finally {
      setAddingRepo(false);
    }
  };

  const handleDeleteRepository = async (repoId: string) => {
    if (!selectedProjectId) return;

    setDeletingRepoId(repoId);
    setRepoError(null);
    try {
      await projectsApi.deleteRepository(selectedProjectId, repoId);
      setRepositories((prev) => prev.filter((r) => r.id !== repoId));
      queryClient.invalidateQueries({
        queryKey: ['projectRepositories', selectedProjectId],
      });
      queryClient.invalidateQueries({
        queryKey: ['repos'],
      });
      queryClient.invalidateQueries({
        queryKey: repoBranchKeys.byRepo(repoId),
      });
    } catch (err) {
      setRepoError(
        err instanceof Error ? err.message : 'Failed to delete repository'
      );
    } finally {
      setDeletingRepoId(null);
    }
  };

  const { updateProject } = useProjectMutations({
    onUpdateSuccess: (updatedProject: Project) => {
      // Update local state with fresh data from server
      setSelectedProject(updatedProject);
      setDraft(projectToFormState(updatedProject));
      setSuccess(true);
      setTimeout(() => setSuccess(false), 3000);
      setSaving(false);
    },
    onUpdateError: (err) => {
      setError(
        err instanceof Error ? err.message : 'Failed to save project settings'
      );
      setSaving(false);
    },
  });

  const handleSave = async () => {
    if (!draft || !selectedProject) return;

    setSaving(true);
    setError(null);
    setSuccess(false);

    try {
      const updateData: UpdateProject = {
        name: draft.name.trim(),
        inbox_prd_template: draft.inbox_prd_template ?? '',
      };

      updateProject.mutate({
        projectId: selectedProject.id,
        data: updateData,
      });
    } catch (err) {
      setError(t('settings.projects.save.error'));
      console.error('Error saving project settings:', err);
      setSaving(false);
    }
  };

  const handleSaveIntegrations = async () => {
    if (!selectedProjectId) return;
    setSavingIntegrations(true);
    setIntegrationsError(null);
    try {
      const payload: UpdateProjectIntegrationsRequest = {
        linear_api_key: null,
        linear_webhook_secret: null,
        intercom_access_token: null,
        intercom_webhook_secret: null,
        modjo_api_key: null,
        modjo_webhook_secret: null,
        posthog_webhook_secret: null,
        sentry_webhook_secret: null,
        posthog_api_key: null,
        posthog_host: posthogHost || null,
        posthog_project_id: posthogProjectId || null,
        sentry_api_token: null,
        sentry_org_slug: sentryOrgSlug || null,
        sentry_project_slug: sentryProjectSlug || null,
        slack_bot_token: null,
        slack_signing_secret: null,
        slack_channel_id: slackChannelId || null,
        clear_linear_api_key: null,
        clear_linear_webhook_secret: null,
        clear_intercom_access_token: null,
        clear_intercom_webhook_secret: null,
        clear_modjo_api_key: null,
        clear_modjo_webhook_secret: null,
        clear_posthog_webhook_secret: null,
        clear_sentry_webhook_secret: null,
        clear_posthog_api_key: null,
        clear_sentry_api_token: null,
        clear_slack_bot_token: null,
        clear_slack_signing_secret: null,
        linear_team_id: linearTeamId || null,
        linear_state_id_todo: linearStateTodo || null,
        linear_state_id_inprogress: linearStateInProgress || null,
        linear_state_id_inreview: linearStateInReview || null,
        linear_state_id_done: linearStateDone || null,
        linear_state_id_cancelled: linearStateCancelled || null,
        intercom_admin_id: intercomAdminId || null,
      };

      if (linearApiKey.trim()) {
        payload.linear_api_key = linearApiKey.trim();
      }
      if (linearWebhookSecret.trim()) {
        payload.linear_webhook_secret = linearWebhookSecret.trim();
      }
      if (intercomAccessToken.trim()) {
        payload.intercom_access_token = intercomAccessToken.trim();
      }
      if (intercomWebhookSecret.trim()) {
        payload.intercom_webhook_secret = intercomWebhookSecret.trim();
      }
      if (modjoApiKey.trim()) {
        payload.modjo_api_key = modjoApiKey.trim();
      }
      if (modjoWebhookSecret.trim()) {
        payload.modjo_webhook_secret = modjoWebhookSecret.trim();
      }
      if (posthogWebhookSecret.trim()) {
        payload.posthog_webhook_secret = posthogWebhookSecret.trim();
      }
      if (sentryWebhookSecret.trim()) {
        payload.sentry_webhook_secret = sentryWebhookSecret.trim();
      }
      if (posthogApiKey.trim()) {
        payload.posthog_api_key = posthogApiKey.trim();
      }
      if (sentryApiToken.trim()) {
        payload.sentry_api_token = sentryApiToken.trim();
      }
      if (slackBotToken.trim()) {
        payload.slack_bot_token = slackBotToken.trim();
      }
      if (slackSigningSecret.trim()) {
        payload.slack_signing_secret = slackSigningSecret.trim();
      }

      const updated = await projectIntegrationsApi.update(
        selectedProjectId,
        payload
      );
      setIntegrations(updated);
      setLinearApiKey('');
      setLinearWebhookSecret('');
      setIntercomAccessToken('');
      setIntercomWebhookSecret('');
      setModjoApiKey('');
      setModjoWebhookSecret('');
      setPosthogWebhookSecret('');
      setSentryWebhookSecret('');
      setPosthogApiKey('');
      setSentryApiToken('');
      setSlackBotToken('');
      setSlackSigningSecret('');
      setSuccess(true);
      setTimeout(() => setSuccess(false), 3000);
    } catch (err) {
      setIntegrationsError(
        err instanceof Error ? err.message : 'Failed to save integrations'
      );
    } finally {
      setSavingIntegrations(false);
    }
  };

  const handleDiscard = () => {
    if (!selectedProject) return;
    setDraft(projectToFormState(selectedProject));
  };

  const updateDraft = (updates: Partial<ProjectFormState>) => {
    setDraft((prev) => {
      if (!prev) return prev;
      return { ...prev, ...updates };
    });
  };

  if (projectsLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="h-8 w-8 animate-spin" />
        <span className="ml-2">{t('settings.projects.loading')}</span>
      </div>
    );
  }

  if (projectsError) {
    return (
      <div className="py-8">
        <Alert variant="destructive">
          <AlertDescription>
            {projectsError instanceof Error
              ? projectsError.message
              : t('settings.projects.loadError')}
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {success && (
        <Alert variant="success">
          <AlertDescription className="font-medium">
            {t('settings.projects.save.success')}
          </AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.projects.title')}</CardTitle>
          <CardDescription>
            {t('settings.projects.description')}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="project-selector">
              {t('settings.projects.selector.label')}
            </Label>
            <Select
              value={selectedProjectId}
              onValueChange={handleProjectSelect}
            >
              <SelectTrigger id="project-selector">
                <SelectValue
                  placeholder={t('settings.projects.selector.placeholder')}
                />
              </SelectTrigger>
              <SelectContent>
                {projects && projects.length > 0 ? (
                  projects.map((project) => (
                    <SelectItem key={project.id} value={project.id}>
                      {project.name}
                    </SelectItem>
                  ))
                ) : (
                  <SelectItem value="no-projects" disabled>
                    {t('settings.projects.selector.noProjects')}
                  </SelectItem>
                )}
              </SelectContent>
            </Select>
            <p className="text-sm text-muted-foreground">
              {t('settings.projects.selector.helper')}
            </p>
          </div>
        </CardContent>
      </Card>

      {selectedProject && draft && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>{t('settings.projects.general.title')}</CardTitle>
              <CardDescription>
                {t('settings.projects.general.description')}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="project-name">
                  {t('settings.projects.general.name.label')}
                </Label>
                <Input
                  id="project-name"
                  type="text"
                  value={draft.name}
                  onChange={(e) => updateDraft({ name: e.target.value })}
                  placeholder={t('settings.projects.general.name.placeholder')}
                  required
                />
                <p className="text-sm text-muted-foreground">
                  {t('settings.projects.general.name.helper')}
                </p>
              </div>

              {/* Save Button */}
              <div className="flex items-center justify-between pt-4 border-t">
                {hasUnsavedChanges ? (
                  <span className="text-sm text-muted-foreground">
                    {t('settings.projects.save.unsavedChanges')}
                  </span>
                ) : (
                  <span />
                )}
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    onClick={handleDiscard}
                    disabled={saving || !hasUnsavedChanges}
                  >
                    {t('settings.projects.save.discard')}
                  </Button>
                  <Button
                    onClick={handleSave}
                    disabled={saving || !hasUnsavedChanges}
                  >
                    {saving ? (
                      <>
                        <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                        {t('settings.projects.save.saving')}
                      </>
                    ) : (
                      t('settings.projects.save.button')
                    )}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>{t('settings.projects.prdTemplate.title')}</CardTitle>
              <CardDescription>
                {t('settings.projects.prdTemplate.description')}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center space-x-2">
                <Checkbox
                  id="use-custom-inbox-prd-template"
                  checked={draft.inbox_prd_template != null}
                  onCheckedChange={(checked: boolean) => {
                    if (checked) {
                      updateDraft({
                        inbox_prd_template: DEFAULT_INBOX_PRD_TEMPLATE,
                      });
                    } else {
                      updateDraft({ inbox_prd_template: null });
                    }
                  }}
                />
                <Label
                  htmlFor="use-custom-inbox-prd-template"
                  className="cursor-pointer"
                >
                  {t('settings.projects.prdTemplate.useCustom')}
                </Label>
              </div>
              <div className="space-y-2">
                <textarea
                  id="inbox-prd-template"
                  className={`flex min-h-[160px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 ${
                    draft.inbox_prd_template == null
                      ? 'opacity-50 cursor-not-allowed'
                      : ''
                  }`}
                  value={
                    draft.inbox_prd_template ?? DEFAULT_INBOX_PRD_TEMPLATE
                  }
                  disabled={draft.inbox_prd_template == null}
                  onChange={(e) =>
                    updateDraft({
                      inbox_prd_template: e.target.value,
                    })
                  }
                />
                <p className="text-sm text-muted-foreground">
                  {t('settings.projects.prdTemplate.helper')}
                </p>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Integrations</CardTitle>
              <CardDescription>
                Configure integrations for the Inbox. Enable an integration to configure it.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {integrationsError && (
                <Alert variant="destructive">
                  <AlertDescription>{integrationsError}</AlertDescription>
                </Alert>
              )}

              {integrationsLoading && (
                <div className="flex items-center text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin mr-2" />
                  Loading integrations...
                </div>
              )}

              {integrations && (
                <>
                  {/* Linear Integration */}
                  <div className="border rounded-lg">
                    <div
                      role="button"
                      tabIndex={0}
                      onClick={() => setLinearEnabled(!linearEnabled)}
                      onKeyDown={(e) => e.key === 'Enter' && setLinearEnabled(!linearEnabled)}
                      className="w-full flex items-center justify-between p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                    >
                      <div className="flex items-center gap-3">
                        <Switch
                          checked={linearEnabled}
                          onCheckedChange={setLinearEnabled}
                          onClick={(e) => e.stopPropagation()}
                        />
                        <div className="text-left">
                          <div className="font-medium">Linear</div>
                          <div className="text-sm text-muted-foreground">
                            Sync tasks with Linear issues
                          </div>
                        </div>
                      </div>
                      <ChevronDown
                        className={`h-5 w-5 text-muted-foreground transition-transform ${
                          linearEnabled ? 'rotate-180' : ''
                        }`}
                      />
                    </div>
                    {linearEnabled && (
                      <div className="border-t p-4 space-y-4">
                        <div className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-2">
                            <Label>API Key</Label>
                            <Input
                              type="password"
                              value={linearApiKey}
                              onChange={(e) => setLinearApiKey(e.target.value)}
                              placeholder={
                                integrations.linear_api_key_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Webhook Secret</Label>
                            <Input
                              type="password"
                              value={linearWebhookSecret}
                              onChange={(e) => setLinearWebhookSecret(e.target.value)}
                              placeholder={
                                integrations.linear_webhook_secret_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Team</Label>
                            <Select
                              value={linearTeamId}
                              onValueChange={setLinearTeamId}
                            >
                              <SelectTrigger>
                                <SelectValue placeholder="Select a team" />
                              </SelectTrigger>
                              <SelectContent>
                                {linearTeamsQuery.data?.length ? (
                                  linearTeamsQuery.data.map((team) => (
                                    <SelectItem key={team.id} value={team.id}>
                                      {team.name}
                                    </SelectItem>
                                  ))
                                ) : (
                                  <SelectItem value="no-teams" disabled>
                                    No teams available
                                  </SelectItem>
                                )}
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <Label>Backlog (Todo) State</Label>
                            <Select
                              value={linearStateTodo}
                              onValueChange={setLinearStateTodo}
                            >
                              <SelectTrigger>
                                <SelectValue placeholder="Select state" />
                              </SelectTrigger>
                              <SelectContent>
                                {linearStatesQuery.data?.length ? (
                                  linearStatesQuery.data.map((state) => (
                                    <SelectItem key={state.id} value={state.id}>
                                      {state.name}
                                    </SelectItem>
                                  ))
                                ) : (
                                  <SelectItem value="no-states" disabled>
                                    No states available
                                  </SelectItem>
                                )}
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <Label>In Progress State</Label>
                            <Select
                              value={linearStateInProgress}
                              onValueChange={setLinearStateInProgress}
                            >
                              <SelectTrigger>
                                <SelectValue placeholder="Select state" />
                              </SelectTrigger>
                              <SelectContent>
                                {linearStatesQuery.data?.length ? (
                                  linearStatesQuery.data.map((state) => (
                                    <SelectItem key={state.id} value={state.id}>
                                      {state.name}
                                    </SelectItem>
                                  ))
                                ) : (
                                  <SelectItem value="no-states" disabled>
                                    No states available
                                  </SelectItem>
                                )}
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <Label>In Review State</Label>
                            <Select
                              value={linearStateInReview}
                              onValueChange={setLinearStateInReview}
                            >
                              <SelectTrigger>
                                <SelectValue placeholder="Select state" />
                              </SelectTrigger>
                              <SelectContent>
                                {linearStatesQuery.data?.length ? (
                                  linearStatesQuery.data.map((state) => (
                                    <SelectItem key={state.id} value={state.id}>
                                      {state.name}
                                    </SelectItem>
                                  ))
                                ) : (
                                  <SelectItem value="no-states" disabled>
                                    No states available
                                  </SelectItem>
                                )}
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <Label>Done State</Label>
                            <Select
                              value={linearStateDone}
                              onValueChange={setLinearStateDone}
                            >
                              <SelectTrigger>
                                <SelectValue placeholder="Select state" />
                              </SelectTrigger>
                              <SelectContent>
                                {linearStatesQuery.data?.length ? (
                                  linearStatesQuery.data.map((state) => (
                                    <SelectItem key={state.id} value={state.id}>
                                      {state.name}
                                    </SelectItem>
                                  ))
                                ) : (
                                  <SelectItem value="no-states" disabled>
                                    No states available
                                  </SelectItem>
                                )}
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <Label>Cancelled State</Label>
                            <Select
                              value={linearStateCancelled}
                              onValueChange={setLinearStateCancelled}
                            >
                              <SelectTrigger>
                                <SelectValue placeholder="Select state" />
                              </SelectTrigger>
                              <SelectContent>
                                {linearStatesQuery.data?.length ? (
                                  linearStatesQuery.data.map((state) => (
                                    <SelectItem key={state.id} value={state.id}>
                                      {state.name}
                                    </SelectItem>
                                  ))
                                ) : (
                                  <SelectItem value="no-states" disabled>
                                    No states available
                                  </SelectItem>
                                )}
                              </SelectContent>
                            </Select>
                          </div>
                        </div>
                        {webhookUrls && (
                          <div className="text-sm text-muted-foreground break-all pt-2 border-t">
                            Webhook URL: {webhookUrls.linear}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Intercom Integration */}
                  <div className="border rounded-lg">
                    <div
                      role="button"
                      tabIndex={0}
                      onClick={() => setIntercomEnabled(!intercomEnabled)}
                      onKeyDown={(e) => e.key === 'Enter' && setIntercomEnabled(!intercomEnabled)}
                      className="w-full flex items-center justify-between p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                    >
                      <div className="flex items-center gap-3">
                        <Switch
                          checked={intercomEnabled}
                          onCheckedChange={setIntercomEnabled}
                          onClick={(e) => e.stopPropagation()}
                        />
                        <div className="text-left">
                          <div className="font-medium">Intercom</div>
                          <div className="text-sm text-muted-foreground">
                            Create tasks from Intercom conversations
                          </div>
                        </div>
                      </div>
                      <ChevronDown
                        className={`h-5 w-5 text-muted-foreground transition-transform ${
                          intercomEnabled ? 'rotate-180' : ''
                        }`}
                      />
                    </div>
                    {intercomEnabled && (
                      <div className="border-t p-4 space-y-4">
                        <div className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-2">
                            <Label>Access Token</Label>
                            <Input
                              type="password"
                              value={intercomAccessToken}
                              onChange={(e) => setIntercomAccessToken(e.target.value)}
                              placeholder={
                                integrations.intercom_access_token_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Admin ID</Label>
                            <Input
                              value={intercomAdminId}
                              onChange={(e) => setIntercomAdminId(e.target.value)}
                              placeholder="Admin ID for internal notes"
                            />
                          </div>
                          <div className="space-y-2 md:col-span-2">
                            <Label>Webhook Secret</Label>
                            <Input
                              type="password"
                              value={intercomWebhookSecret}
                              onChange={(e) => setIntercomWebhookSecret(e.target.value)}
                              placeholder={
                                integrations.intercom_webhook_secret_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                        </div>
                        {webhookUrls && (
                          <div className="text-sm text-muted-foreground break-all pt-2 border-t">
                            Webhook URL: {webhookUrls.intercom}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Modjo Integration */}
                  <div className="border rounded-lg">
                    <div
                      role="button"
                      tabIndex={0}
                      onClick={() => setModjoEnabled(!modjoEnabled)}
                      onKeyDown={(e) => e.key === 'Enter' && setModjoEnabled(!modjoEnabled)}
                      className="w-full flex items-center justify-between p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                    >
                      <div className="flex items-center gap-3">
                        <Switch
                          checked={modjoEnabled}
                          onCheckedChange={setModjoEnabled}
                          onClick={(e) => e.stopPropagation()}
                        />
                        <div className="text-left">
                          <div className="font-medium">Modjo</div>
                          <div className="text-sm text-muted-foreground">
                            Import call insights from Modjo
                          </div>
                        </div>
                      </div>
                      <ChevronDown
                        className={`h-5 w-5 text-muted-foreground transition-transform ${
                          modjoEnabled ? 'rotate-180' : ''
                        }`}
                      />
                    </div>
                    {modjoEnabled && (
                      <div className="border-t p-4 space-y-4">
                        <div className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-2">
                            <Label>API Key</Label>
                            <Input
                              type="password"
                              value={modjoApiKey}
                              onChange={(e) => setModjoApiKey(e.target.value)}
                              placeholder={
                                integrations.modjo_api_key_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Webhook Secret</Label>
                            <Input
                              type="password"
                              value={modjoWebhookSecret}
                              onChange={(e) => setModjoWebhookSecret(e.target.value)}
                              placeholder={
                                integrations.modjo_webhook_secret_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                        </div>
                        {webhookUrls && (
                          <div className="text-sm text-muted-foreground break-all pt-2 border-t">
                            Webhook URL: {webhookUrls.modjo}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* PostHog Integration */}
                  <div className="border rounded-lg">
                    <div
                      role="button"
                      tabIndex={0}
                      onClick={() => setPosthogEnabled(!posthogEnabled)}
                      onKeyDown={(e) => e.key === 'Enter' && setPosthogEnabled(!posthogEnabled)}
                      className="w-full flex items-center justify-between p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                    >
                      <div className="flex items-center gap-3">
                        <Switch
                          checked={posthogEnabled}
                          onCheckedChange={setPosthogEnabled}
                          onClick={(e) => e.stopPropagation()}
                        />
                        <div className="text-left">
                          <div className="font-medium">PostHog</div>
                          <div className="text-sm text-muted-foreground">
                            Create tasks from PostHog events
                          </div>
                        </div>
                      </div>
                      <ChevronDown
                        className={`h-5 w-5 text-muted-foreground transition-transform ${
                          posthogEnabled ? 'rotate-180' : ''
                        }`}
                      />
                    </div>
                    {posthogEnabled && (
                      <div className="border-t p-4 space-y-4">
                        <div className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-2">
                            <Label>API Key</Label>
                            <Input
                              type="password"
                              value={posthogApiKey}
                              onChange={(e) => setPosthogApiKey(e.target.value)}
                              placeholder={
                                integrations.posthog_api_key_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Webhook Secret</Label>
                            <Input
                              type="password"
                              value={posthogWebhookSecret}
                              onChange={(e) => setPosthogWebhookSecret(e.target.value)}
                              placeholder={
                                integrations.posthog_webhook_secret_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Host</Label>
                            <Input
                              value={posthogHost}
                              onChange={(e) => setPosthogHost(e.target.value)}
                              placeholder="https://app.posthog.com"
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Project ID</Label>
                            <Input
                              value={posthogProjectId}
                              onChange={(e) => setPosthogProjectId(e.target.value)}
                              placeholder="Project ID for event API"
                            />
                          </div>
                        </div>
                        {webhookUrls && (
                          <div className="text-sm text-muted-foreground break-all pt-2 border-t">
                            Webhook URL: {webhookUrls.posthog}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Sentry Integration */}
                  <div className="border rounded-lg">
                    <div
                      role="button"
                      tabIndex={0}
                      onClick={() => setSentryEnabled(!sentryEnabled)}
                      onKeyDown={(e) => e.key === 'Enter' && setSentryEnabled(!sentryEnabled)}
                      className="w-full flex items-center justify-between p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                    >
                      <div className="flex items-center gap-3">
                        <Switch
                          checked={sentryEnabled}
                          onCheckedChange={setSentryEnabled}
                          onClick={(e) => e.stopPropagation()}
                        />
                        <div className="text-left">
                          <div className="font-medium">Sentry</div>
                          <div className="text-sm text-muted-foreground">
                            Create tasks from Sentry issues
                          </div>
                        </div>
                      </div>
                      <ChevronDown
                        className={`h-5 w-5 text-muted-foreground transition-transform ${
                          sentryEnabled ? 'rotate-180' : ''
                        }`}
                      />
                    </div>
                    {sentryEnabled && (
                      <div className="border-t p-4 space-y-4">
                        <div className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-2">
                            <Label>API Token</Label>
                            <Input
                              type="password"
                              value={sentryApiToken}
                              onChange={(e) => setSentryApiToken(e.target.value)}
                              placeholder={
                                integrations.sentry_api_token_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Webhook Secret</Label>
                            <Input
                              type="password"
                              value={sentryWebhookSecret}
                              onChange={(e) => setSentryWebhookSecret(e.target.value)}
                              placeholder={
                                integrations.sentry_webhook_secret_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Org Slug</Label>
                            <Input
                              value={sentryOrgSlug}
                              onChange={(e) => setSentryOrgSlug(e.target.value)}
                              placeholder="your-org"
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Project Slug</Label>
                            <Input
                              value={sentryProjectSlug}
                              onChange={(e) => setSentryProjectSlug(e.target.value)}
                              placeholder="your-project"
                            />
                          </div>
                        </div>
                        {webhookUrls && (
                          <div className="text-sm text-muted-foreground break-all pt-2 border-t">
                            Webhook URL: {webhookUrls.sentry}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Slack Integration */}
                  <div className="border rounded-lg">
                    <div
                      role="button"
                      tabIndex={0}
                      onClick={() => setSlackEnabled(!slackEnabled)}
                      onKeyDown={(e) => e.key === 'Enter' && setSlackEnabled(!slackEnabled)}
                      className="w-full flex items-center justify-between p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                    >
                      <div className="flex items-center gap-3">
                        <Switch
                          checked={slackEnabled}
                          onCheckedChange={setSlackEnabled}
                          onClick={(e) => e.stopPropagation()}
                        />
                        <div className="text-left">
                          <div className="font-medium">Slack</div>
                          <div className="text-sm text-muted-foreground">
                            Post PRDs to Slack with interactive buttons
                          </div>
                        </div>
                      </div>
                      <ChevronDown
                        className={`h-5 w-5 text-muted-foreground transition-transform ${
                          slackEnabled ? 'rotate-180' : ''
                        }`}
                      />
                    </div>
                    {slackEnabled && (
                      <div className="border-t p-4 space-y-4">
                        <div className="rounded-md bg-muted/50 p-3 text-sm">
                          <div className="font-medium mb-2">Required Bot Scopes</div>
                          <div className="text-muted-foreground space-y-1">
                            <div><code className="bg-muted px-1 rounded">chat:write</code> - Post messages to channels</div>
                            <div><code className="bg-muted px-1 rounded">chat:write.public</code> - Post to public channels without joining</div>
                            <div><code className="bg-muted px-1 rounded">commands</code> - Handle slash commands</div>
                            <div><code className="bg-muted px-1 rounded">im:write</code> - Send direct messages to users</div>
                          </div>
                        </div>
                        <div className="grid gap-4 md:grid-cols-2">
                          <div className="space-y-2">
                            <Label>Bot Token</Label>
                            <Input
                              type="password"
                              value={slackBotToken}
                              onChange={(e) => setSlackBotToken(e.target.value)}
                              placeholder={
                                integrations.slack_bot_token_configured
                                  ? 'Configured'
                                  : 'xoxb-...'
                              }
                            />
                            <p className="text-xs text-muted-foreground">
                              Bot User OAuth Token from your Slack app
                            </p>
                          </div>
                          <div className="space-y-2">
                            <Label>Signing Secret</Label>
                            <Input
                              type="password"
                              value={slackSigningSecret}
                              onChange={(e) => setSlackSigningSecret(e.target.value)}
                              placeholder={
                                integrations.slack_signing_secret_configured
                                  ? 'Configured'
                                  : 'Not set'
                              }
                            />
                            <p className="text-xs text-muted-foreground">
                              Used to verify webhook requests from Slack
                            </p>
                          </div>
                          <div className="space-y-2 md:col-span-2">
                            <Label>Channel ID</Label>
                            <Input
                              value={slackChannelId}
                              onChange={(e) => setSlackChannelId(e.target.value)}
                              placeholder="C01234567"
                            />
                            <p className="text-xs text-muted-foreground">
                              Channel ID where PRDs will be posted (right-click channel → View channel details → Copy ID)
                            </p>
                          </div>
                        </div>
                        {webhookUrls && (
                          <div className="space-y-1 text-sm text-muted-foreground break-all pt-2 border-t">
                            <div>Slash Command URL: {webhookUrls.slack_commands}</div>
                            <div>Interactivity URL: {webhookUrls.slack_interactivity}</div>
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Manual Webhook URL */}
                  {webhookUrls && (
                    <div className="border rounded-lg p-4 space-y-2">
                      <div className="font-medium">Manual Webhook</div>
                      <p className="text-sm text-muted-foreground">
                        Use this URL to create inbox items programmatically from external sources.
                      </p>
                      <div className="text-sm text-muted-foreground break-all">
                        {webhookUrls.manual}
                      </div>
                      {!integrations.webhook_urls && (
                        <p className="text-xs text-muted-foreground">
                          Showing local URLs. Set `VK_PUBLIC_BASE_URL` in the server to show public URLs.
                        </p>
                      )}
                    </div>
                  )}

                  <div className="flex justify-end pt-4">
                    <Button
                      onClick={handleSaveIntegrations}
                      disabled={savingIntegrations}
                    >
                      {savingIntegrations ? (
                        <>
                          <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                          Saving...
                        </>
                      ) : (
                        'Save integrations'
                      )}
                    </Button>
                  </div>
                </>
              )}
            </CardContent>
          </Card>

          {/* Repositories Section */}
          <Card>
            <CardHeader>
              <CardTitle>Repositories</CardTitle>
              <CardDescription>
                Manage the git repositories in this project
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {repoError && (
                <Alert variant="destructive">
                  <AlertDescription>{repoError}</AlertDescription>
                </Alert>
              )}

              {loadingRepos ? (
                <div className="flex items-center justify-center py-4">
                  <Loader2 className="h-5 w-5 animate-spin" />
                  <span className="ml-2 text-sm text-muted-foreground">
                    Loading repositories...
                  </span>
                </div>
              ) : (
                <div className="space-y-2">
                  {repositories.map((repo) => (
                    <div
                      key={repo.id}
                      className="flex items-center justify-between p-3 border rounded-md hover:bg-muted/50 cursor-pointer transition-colors"
                      onClick={() =>
                        navigate(`/settings/repos?repoId=${repo.id}`)
                      }
                    >
                      <div className="min-w-0 flex-1">
                        <div className="font-medium">{repo.display_name}</div>
                        <div className="text-sm text-muted-foreground truncate">
                          {repo.path}
                        </div>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDeleteRepository(repo.id);
                        }}
                        disabled={deletingRepoId === repo.id}
                        title="Delete repository"
                      >
                        {deletingRepoId === repo.id ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : (
                          <Trash2 className="h-4 w-4" />
                        )}
                      </Button>
                    </div>
                  ))}

                  {repositories.length === 0 && !loadingRepos && (
                    <div className="text-center py-4 text-sm text-muted-foreground">
                      No repositories configured
                    </div>
                  )}

                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleAddRepository}
                    disabled={addingRepo}
                    className="w-full"
                  >
                    {addingRepo ? (
                      <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    ) : (
                      <Plus className="h-4 w-4 mr-2" />
                    )}
                    Add Repository
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>

          {/* Dev Server Environment Variables */}
          <Card>
            <CardHeader>
              <CardTitle>Dev Server Environment Variables</CardTitle>
              <CardDescription>
                Configure environment variables for the dev server. Variable names
                are defined in your repository's vibekanban.json file.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {envVarsError && (
                <Alert variant="destructive">
                  <AlertDescription>{envVarsError}</AlertDescription>
                </Alert>
              )}

              {envVarsLoading ? (
                <div className="flex items-center justify-center py-4">
                  <Loader2 className="h-5 w-5 animate-spin" />
                  <span className="ml-2 text-sm text-muted-foreground">
                    Loading environment variables...
                  </span>
                </div>
              ) : envVars.length === 0 ? (
                <div className="text-center py-4 text-sm text-muted-foreground">
                  No environment variables defined. Add an <code className="text-xs bg-muted px-1 py-0.5 rounded">env_vars</code> array to your vibekanban.json to define allowed variables.
                </div>
              ) : (
                <div className="space-y-4">
                  {envVars.map((envVar) => (
                    <div key={envVar.name} className="space-y-2">
                      <Label htmlFor={`env-${envVar.name}`}>{envVar.name}</Label>
                      <div className="flex gap-2">
                        <Input
                          id={`env-${envVar.name}`}
                          type="password"
                          value={envVarInputs[envVar.name] ?? ''}
                          onChange={(e) =>
                            setEnvVarInputs((prev) => ({
                              ...prev,
                              [envVar.name]: e.target.value,
                            }))
                          }
                          placeholder={envVar.configured ? 'Configured' : 'Not set'}
                          className="flex-1"
                        />
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleSaveEnvVar(envVar.name)}
                          disabled={
                            savingEnvVar === envVar.name ||
                            !envVarInputs[envVar.name]?.trim()
                          }
                        >
                          {savingEnvVar === envVar.name ? (
                            <Loader2 className="h-4 w-4 animate-spin" />
                          ) : (
                            'Save'
                          )}
                        </Button>
                        {envVar.configured && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => handleClearEnvVar(envVar.name)}
                            disabled={savingEnvVar === envVar.name}
                            title="Clear value"
                          >
                            {savingEnvVar === envVar.name ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <Trash2 className="h-4 w-4" />
                            )}
                          </Button>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          {/* Sticky Save Button for Project Name */}
          {hasUnsavedChanges && (
            <div className="sticky bottom-0 z-10 bg-background/80 backdrop-blur-sm border-t py-4">
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">
                  {t('settings.projects.save.unsavedChanges')}
                </span>
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    onClick={handleDiscard}
                    disabled={saving}
                  >
                    {t('settings.projects.save.discard')}
                  </Button>
                  <Button onClick={handleSave} disabled={saving}>
                    {saving && (
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    )}
                    {t('settings.projects.save.button')}
                  </Button>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
