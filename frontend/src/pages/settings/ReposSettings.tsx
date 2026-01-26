import { useCallback, useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
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
import { Checkbox } from '@/components/ui/checkbox';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { FileCode, Loader2 } from 'lucide-react';
import { useScriptPlaceholders } from '@/hooks/useScriptPlaceholders';
import { AutoExpandingTextarea } from '@/components/ui/auto-expanding-textarea';
import { MultiFileSearchTextarea } from '@/components/ui/multi-file-search-textarea';
import { repoApi } from '@/lib/api';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { Repo, RepoWithEffectiveConfig, UpdateRepo } from 'shared/types';

/** Badge component to indicate a value comes from vibekanban.json config file */
function ConfigFileBadge() {
  const { t } = useTranslation('settings');
  return (
    <Badge
      variant="secondary"
      className="ml-2 text-xs font-normal gap-1 bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200"
    >
      <FileCode className="h-3 w-3" />
      {t('settings.repos.configFile.badge')}
    </Badge>
  );
}

interface RepoScriptsFormState {
  display_name: string;
  setup_script: string;
  parallel_setup_script: boolean;
  cleanup_script: string;
  copy_files: string;
  dev_server_script: string;
}

function repoToFormState(repo: Repo): RepoScriptsFormState {
  return {
    display_name: repo.display_name,
    setup_script: repo.setup_script ?? '',
    parallel_setup_script: repo.parallel_setup_script,
    cleanup_script: repo.cleanup_script ?? '',
    copy_files: repo.copy_files ?? '',
    dev_server_script: repo.dev_server_script ?? '',
  };
}

export function ReposSettings() {
  const [searchParams, setSearchParams] = useSearchParams();
  const repoIdParam = searchParams.get('repoId') ?? '';
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();

  // Fetch all repos
  const {
    data: repos,
    isLoading: reposLoading,
    error: reposError,
  } = useQuery({
    queryKey: ['repos'],
    queryFn: () => repoApi.list(),
  });

  // Selected repo state
  const [selectedRepoId, setSelectedRepoId] = useState<string>(repoIdParam);
  const [selectedRepo, setSelectedRepo] = useState<Repo | null>(null);

  // Fetch effective config for selected repo (includes vibekanban.json overrides)
  const { data: effectiveConfig, isLoading: effectiveConfigLoading } = useQuery({
    queryKey: ['repo-effective-config', selectedRepoId],
    queryFn: () => repoApi.getEffectiveConfig(selectedRepoId),
    enabled: !!selectedRepoId,
  });

  // Form state
  const [draft, setDraft] = useState<RepoScriptsFormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Get OS-appropriate script placeholders
  const placeholders = useScriptPlaceholders();

  // Helper to check if a field is from config file
  const isFromConfigFile = useCallback(
    (
      field:
        | 'setup_script'
        | 'cleanup_script'
        | 'dev_server_script'
        | 'copy_files'
        | 'parallel_setup_script'
    ): boolean => {
      if (!effectiveConfig) return false;
      return effectiveConfig[`${field}_from_file`] ?? false;
    },
    [effectiveConfig]
  );

  // Check for unsaved changes (compare against effective config if available)
  const hasUnsavedChanges = useMemo(() => {
    if (!draft || !selectedRepo) return false;
    // Compare against effective config values if available
    if (effectiveConfig && effectiveConfig.id === selectedRepo.id) {
      const effectiveState: RepoScriptsFormState = {
        display_name: effectiveConfig.display_name,
        setup_script: effectiveConfig.setup_script ?? '',
        parallel_setup_script: effectiveConfig.parallel_setup_script,
        cleanup_script: effectiveConfig.cleanup_script ?? '',
        copy_files: effectiveConfig.copy_files ?? '',
        dev_server_script: effectiveConfig.dev_server_script ?? '',
      };
      return !isEqual(draft, effectiveState);
    }
    return !isEqual(draft, repoToFormState(selectedRepo));
  }, [draft, selectedRepo, effectiveConfig]);

  // Handle repo selection from dropdown
  const handleRepoSelect = useCallback(
    (id: string) => {
      if (id === selectedRepoId) return;

      if (hasUnsavedChanges) {
        const confirmed = window.confirm(
          t('settings.repos.save.confirmSwitch')
        );
        if (!confirmed) return;
        setDraft(null);
        setSelectedRepo(null);
        setSuccess(false);
        setError(null);
      }

      setSelectedRepoId(id);
      if (id) {
        setSearchParams({ repoId: id });
      } else {
        setSearchParams({});
      }
    },
    [hasUnsavedChanges, selectedRepoId, setSearchParams, t]
  );

  // Sync selectedRepoId when URL changes
  useEffect(() => {
    if (repoIdParam === selectedRepoId) return;

    if (hasUnsavedChanges) {
      const confirmed = window.confirm(t('settings.repos.save.confirmSwitch'));
      if (!confirmed) {
        if (selectedRepoId) {
          setSearchParams({ repoId: selectedRepoId });
        } else {
          setSearchParams({});
        }
        return;
      }
      setDraft(null);
      setSelectedRepo(null);
      setSuccess(false);
      setError(null);
    }

    setSelectedRepoId(repoIdParam);
  }, [repoIdParam, hasUnsavedChanges, selectedRepoId, setSearchParams, t]);

  // Helper to create form state from effective config
  const effectiveConfigToFormState = useCallback(
    (config: RepoWithEffectiveConfig): RepoScriptsFormState => {
      return {
        display_name: config.display_name,
        setup_script: config.setup_script ?? '',
        parallel_setup_script: config.parallel_setup_script,
        cleanup_script: config.cleanup_script ?? '',
        copy_files: config.copy_files ?? '',
        dev_server_script: config.dev_server_script ?? '',
      };
    },
    []
  );

  // Populate draft from server data
  useEffect(() => {
    if (!repos) return;

    const nextRepo = selectedRepoId
      ? repos.find((r) => r.id === selectedRepoId)
      : null;

    setSelectedRepo((prev) =>
      prev?.id === nextRepo?.id ? prev : (nextRepo ?? null)
    );

    if (!nextRepo) {
      if (!hasUnsavedChanges) setDraft(null);
      return;
    }

    if (hasUnsavedChanges) return;

    // Wait for effectiveConfig to load before setting draft to avoid
    // showing database values before vibekanban.json overrides are applied
    if (selectedRepoId && effectiveConfigLoading) {
      return;
    }

    // Use effective config values if available, otherwise fall back to repo values
    if (effectiveConfig && effectiveConfig.id === nextRepo.id) {
      setDraft(effectiveConfigToFormState(effectiveConfig));
    } else {
      setDraft(repoToFormState(nextRepo));
    }
  }, [repos, selectedRepoId, hasUnsavedChanges, effectiveConfig, effectiveConfigLoading, effectiveConfigToFormState]);

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

  const handleSave = async () => {
    if (!draft || !selectedRepo) return;

    setSaving(true);
    setError(null);
    setSuccess(false);

    try {
      const updateData: UpdateRepo = {
        display_name: draft.display_name.trim() || null,
        setup_script: draft.setup_script.trim() || null,
        cleanup_script: draft.cleanup_script.trim() || null,
        copy_files: draft.copy_files.trim() || null,
        parallel_setup_script: draft.parallel_setup_script,
        dev_server_script: draft.dev_server_script.trim() || null,
      };

      const updatedRepo = await repoApi.update(selectedRepo.id, updateData);
      setSelectedRepo(updatedRepo);
      queryClient.setQueryData(['repos'], (old: Repo[] | undefined) =>
        old?.map((r) => (r.id === updatedRepo.id ? updatedRepo : r))
      );
      // Invalidate effective-config to refetch with vibekanban.json overrides
      await queryClient.invalidateQueries({ queryKey: ['repo-effective-config', selectedRepo.id] });
      setSuccess(true);
      setTimeout(() => setSuccess(false), 3000);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('settings.repos.save.error')
      );
    } finally {
      setSaving(false);
    }
  };

  const handleDiscard = () => {
    if (!selectedRepo) return;
    // Use effective config values (includes vibekanban.json) if available
    if (effectiveConfig && effectiveConfig.id === selectedRepo.id) {
      setDraft(effectiveConfigToFormState(effectiveConfig));
    } else {
      setDraft(repoToFormState(selectedRepo));
    }
  };

  const updateDraft = (updates: Partial<RepoScriptsFormState>) => {
    setDraft((prev) => {
      if (!prev) return prev;
      return { ...prev, ...updates };
    });
  };

  if (reposLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="h-8 w-8 animate-spin" />
        <span className="ml-2">{t('settings.repos.loading')}</span>
      </div>
    );
  }

  if (reposError) {
    return (
      <div className="py-8">
        <Alert variant="destructive">
          <AlertDescription>
            {reposError instanceof Error
              ? reposError.message
              : t('settings.repos.loadError')}
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
            {t('settings.repos.save.success')}
          </AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.repos.title')}</CardTitle>
          <CardDescription>{t('settings.repos.description')}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="repo-selector">
              {t('settings.repos.selector.label')}
            </Label>
            <Select value={selectedRepoId} onValueChange={handleRepoSelect}>
              <SelectTrigger id="repo-selector">
                <SelectValue
                  placeholder={t('settings.repos.selector.placeholder')}
                />
              </SelectTrigger>
              <SelectContent>
                {repos && repos.length > 0 ? (
                  repos.map((repo) => (
                    <SelectItem key={repo.id} value={repo.id}>
                      {repo.display_name}
                    </SelectItem>
                  ))
                ) : (
                  <SelectItem value="no-repos" disabled>
                    {t('settings.repos.selector.noRepos')}
                  </SelectItem>
                )}
              </SelectContent>
            </Select>
            <p className="text-sm text-muted-foreground">
              {t('settings.repos.selector.helper')}
            </p>
          </div>
        </CardContent>
      </Card>

      {selectedRepo && draft && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>{t('settings.repos.general.title')}</CardTitle>
              <CardDescription>
                {t('settings.repos.general.description')}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="display-name">
                  {t('settings.repos.general.displayName.label')}
                </Label>
                <Input
                  id="display-name"
                  type="text"
                  value={draft.display_name}
                  onChange={(e) =>
                    updateDraft({ display_name: e.target.value })
                  }
                  placeholder={t(
                    'settings.repos.general.displayName.placeholder'
                  )}
                />
                <p className="text-sm text-muted-foreground">
                  {t('settings.repos.general.displayName.helper')}
                </p>
              </div>

              <div className="space-y-2">
                <Label>{t('settings.repos.general.path.label')}</Label>
                <div className="text-sm text-muted-foreground font-mono bg-muted px-3 py-2 rounded-md">
                  {selectedRepo.path}
                </div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>{t('settings.repos.scripts.title')}</CardTitle>
              <CardDescription>
                {t('settings.repos.scripts.description')}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {/* Config file notice */}
              {effectiveConfig?.has_config_file && (
                <Alert className="bg-blue-50 border-blue-200 dark:bg-blue-950 dark:border-blue-800">
                  <FileCode className="h-4 w-4 text-blue-600 dark:text-blue-400" />
                  <AlertDescription className="text-blue-800 dark:text-blue-200">
                    {t('settings.repos.configFile.notice')}
                  </AlertDescription>
                </Alert>
              )}

              <div className="space-y-2">
                <div className="flex items-center">
                  <Label htmlFor="dev-server-script">
                    {t('settings.repos.scripts.devServer.label')}
                  </Label>
                  {isFromConfigFile('dev_server_script') && <ConfigFileBadge />}
                </div>
                <AutoExpandingTextarea
                  id="dev-server-script"
                  value={draft.dev_server_script}
                  onChange={(e) =>
                    updateDraft({
                      dev_server_script: e.target.value,
                    })
                  }
                  placeholder={placeholders.dev}
                  maxRows={12}
                  disabled={isFromConfigFile('dev_server_script')}
                  className={`w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono ${
                    isFromConfigFile('dev_server_script')
                      ? 'opacity-75 cursor-not-allowed bg-muted'
                      : ''
                  }`}
                />
                <p className="text-sm text-muted-foreground">
                  {isFromConfigFile('dev_server_script')
                    ? t('settings.repos.configFile.fieldReadOnly')
                    : t('settings.repos.scripts.devServer.helper')}
                </p>
              </div>

              <div className="space-y-2">
                <div className="flex items-center">
                  <Label htmlFor="setup-script">
                    {t('settings.repos.scripts.setup.label')}
                  </Label>
                  {isFromConfigFile('setup_script') && <ConfigFileBadge />}
                </div>
                <AutoExpandingTextarea
                  id="setup-script"
                  value={draft.setup_script}
                  onChange={(e) =>
                    updateDraft({ setup_script: e.target.value })
                  }
                  placeholder={placeholders.setup}
                  maxRows={12}
                  disabled={isFromConfigFile('setup_script')}
                  className={`w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono ${
                    isFromConfigFile('setup_script')
                      ? 'opacity-75 cursor-not-allowed bg-muted'
                      : ''
                  }`}
                />
                <p className="text-sm text-muted-foreground">
                  {isFromConfigFile('setup_script')
                    ? t('settings.repos.configFile.fieldReadOnly')
                    : t('settings.repos.scripts.setup.helper')}
                </p>

                <div className="flex items-center space-x-2 pt-2">
                  <Checkbox
                    id="parallel-setup-script"
                    checked={draft.parallel_setup_script}
                    onCheckedChange={(checked) =>
                      updateDraft({
                        parallel_setup_script: checked === true,
                      })
                    }
                    disabled={
                      !draft.setup_script.trim() ||
                      isFromConfigFile('parallel_setup_script')
                    }
                  />
                  <Label
                    htmlFor="parallel-setup-script"
                    className="text-sm font-normal cursor-pointer"
                  >
                    {t('settings.repos.scripts.setup.parallelLabel')}
                  </Label>
                  {isFromConfigFile('parallel_setup_script') && (
                    <ConfigFileBadge />
                  )}
                </div>
                <p className="text-sm text-muted-foreground pl-6">
                  {isFromConfigFile('parallel_setup_script')
                    ? t('settings.repos.configFile.fieldReadOnly')
                    : t('settings.repos.scripts.setup.parallelHelper')}
                </p>
              </div>

              <div className="space-y-2">
                <div className="flex items-center">
                  <Label htmlFor="cleanup-script">
                    {t('settings.repos.scripts.cleanup.label')}
                  </Label>
                  {isFromConfigFile('cleanup_script') && <ConfigFileBadge />}
                </div>
                <AutoExpandingTextarea
                  id="cleanup-script"
                  value={draft.cleanup_script}
                  onChange={(e) =>
                    updateDraft({
                      cleanup_script: e.target.value,
                    })
                  }
                  placeholder={placeholders.cleanup}
                  maxRows={12}
                  disabled={isFromConfigFile('cleanup_script')}
                  className={`w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono ${
                    isFromConfigFile('cleanup_script')
                      ? 'opacity-75 cursor-not-allowed bg-muted'
                      : ''
                  }`}
                />
                <p className="text-sm text-muted-foreground">
                  {isFromConfigFile('cleanup_script')
                    ? t('settings.repos.configFile.fieldReadOnly')
                    : t('settings.repos.scripts.cleanup.helper')}
                </p>
              </div>

              <div className="space-y-2">
                <div className="flex items-center">
                  <Label htmlFor="copy-files">
                    {t('settings.repos.scripts.copyFiles.label')}
                  </Label>
                  {isFromConfigFile('copy_files') && <ConfigFileBadge />}
                </div>
                {isFromConfigFile('copy_files') ? (
                  <div className="w-full px-3 py-2 border border-input bg-muted text-foreground rounded-md font-mono opacity-75">
                    {draft.copy_files || (
                      <span className="text-muted-foreground italic">
                        {t('settings.repos.scripts.copyFiles.placeholder')}
                      </span>
                    )}
                  </div>
                ) : (
                  <MultiFileSearchTextarea
                    value={draft.copy_files}
                    onChange={(value) => updateDraft({ copy_files: value })}
                    placeholder={t(
                      'settings.repos.scripts.copyFiles.placeholder'
                    )}
                    maxRows={6}
                    repoId={selectedRepo.id}
                    className="w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono"
                  />
                )}
                <p className="text-sm text-muted-foreground">
                  {isFromConfigFile('copy_files')
                    ? t('settings.repos.configFile.fieldReadOnly')
                    : t('settings.repos.scripts.copyFiles.helper')}
                </p>
              </div>

              {/* Save Buttons */}
              <div className="flex items-center justify-between pt-4 border-t">
                {hasUnsavedChanges ? (
                  <span className="text-sm text-muted-foreground">
                    {t('settings.repos.save.unsavedChanges')}
                  </span>
                ) : (
                  <span />
                )}
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    onClick={handleDiscard}
                    disabled={!hasUnsavedChanges || saving}
                  >
                    {t('settings.repos.save.discard')}
                  </Button>
                  <Button
                    onClick={handleSave}
                    disabled={!hasUnsavedChanges || saving}
                  >
                    {saving && (
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    )}
                    {t('settings.repos.save.button')}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}
