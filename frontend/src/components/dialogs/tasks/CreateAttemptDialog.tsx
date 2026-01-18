import { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import RepoBranchSelector from '@/components/tasks/RepoBranchSelector';
import { ExecutorProfileSelector } from '@/components/settings';
import { useAttemptCreation } from '@/hooks/useAttemptCreation';
import {
  useNavigateWithSearch,
  useTask,
  useAttempt,
  useRepoBranchSelection,
  useProjectRepos,
} from '@/hooks';
import { useTaskAttemptsWithSessions } from '@/hooks/useTaskAttempts';
import { useProject } from '@/contexts/ProjectContext';
import { useUserSystem } from '@/components/ConfigProvider';
import { paths } from '@/lib/paths';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';
import type { ExecutorProfileId, BaseCodingAgent } from 'shared/types';
import { useKeySubmitTask, Scope } from '@/keyboard';

export interface CreateAttemptDialogProps {
  taskId: string;
}

const CreateAttemptDialogImpl = NiceModal.create<CreateAttemptDialogProps>(
  ({ taskId }) => {
    const modal = useModal();
    const navigate = useNavigateWithSearch();
    const { projectId } = useProject();
    const { t } = useTranslation('tasks');
    const { profiles, config } = useUserSystem();
    const { createAttempt, isCreating, error } = useAttemptCreation({
      taskId,
      onSuccess: (attempt) => {
        if (projectId) {
          navigate(paths.attempt(projectId, taskId, attempt.id));
        }
      },
    });

    const [userSelectedProfile, setUserSelectedProfile] =
      useState<ExecutorProfileId | null>(null);
    const [ralphEnabled, setRalphEnabled] = useState(false);
    const [ralphMaxIterations, setRalphMaxIterations] = useState(10);
    const [ralphMaxFailures, setRalphMaxFailures] = useState(3);

    const { data: attempts = [], isLoading: isLoadingAttempts } =
      useTaskAttemptsWithSessions(taskId, {
        enabled: modal.visible,
        refetchInterval: 5000,
      });

    const { data: task, isLoading: isLoadingTask } = useTask(taskId, {
      enabled: modal.visible,
    });

    const parentAttemptId = task?.parent_workspace_id ?? undefined;
    const { data: parentAttempt, isLoading: isLoadingParent } = useAttempt(
      parentAttemptId,
      { enabled: modal.visible && !!parentAttemptId }
    );

    const { data: projectRepos = [], isLoading: isLoadingRepos } =
      useProjectRepos(projectId, { enabled: modal.visible });

    const {
      configs: repoBranchConfigs,
      isLoading: isLoadingBranches,
      setRepoBranch,
      getWorkspaceRepoInputs,
      reset: resetBranchSelection,
    } = useRepoBranchSelection({
      repos: projectRepos,
      initialBranch: parentAttempt?.branch,
      enabled: modal.visible && projectRepos.length > 0,
    });

    const latestAttempt = useMemo(() => {
      if (attempts.length === 0) return null;
      return attempts.reduce((latest, attempt) =>
        new Date(attempt.created_at) > new Date(latest.created_at)
          ? attempt
          : latest
      );
    }, [attempts]);

    useEffect(() => {
      if (!modal.visible) {
        setUserSelectedProfile(null);
        resetBranchSelection();
        setRalphEnabled(false);
        setRalphMaxIterations(10);
        setRalphMaxFailures(3);
      }
    }, [modal.visible, resetBranchSelection]);

    const defaultProfile: ExecutorProfileId | null = useMemo(() => {
      if (latestAttempt?.session?.executor) {
        const lastExec = latestAttempt.session.executor as BaseCodingAgent;
        // If the last attempt used the same executor as the user's current preference,
        // we assume they want to use their preferred variant as well.
        // Otherwise, we default to the "default" variant (null) since we don't know
        // what variant they used last time (TaskAttempt doesn't store it).
        const variant =
          config?.executor_profile?.executor === lastExec
            ? config.executor_profile.variant
            : null;

        return {
          executor: lastExec,
          variant,
        };
      }
      return config?.executor_profile ?? null;
    }, [latestAttempt?.session?.executor, config?.executor_profile]);

    const effectiveProfile = userSelectedProfile ?? defaultProfile;

    const isLoadingInitial =
      isLoadingRepos ||
      isLoadingBranches ||
      isLoadingAttempts ||
      isLoadingTask ||
      isLoadingParent;

    const allBranchesSelected = repoBranchConfigs.every(
      (c) => c.targetBranch !== null
    );

    const canCreate = Boolean(
      effectiveProfile &&
        allBranchesSelected &&
        projectRepos.length > 0 &&
        !isCreating &&
        !isLoadingInitial
    );

    const handleCreate = async () => {
      if (
        !effectiveProfile ||
        !allBranchesSelected ||
        projectRepos.length === 0
      )
        return;
      try {
        const repos = getWorkspaceRepoInputs();
        const ralphConfig = ralphEnabled
          ? {
              max_iterations: ralphMaxIterations,
              max_failures: ralphMaxFailures,
            }
          : undefined;

        await createAttempt({
          profile: effectiveProfile,
          repos,
          ralph: ralphConfig,
        });

        modal.hide();
      } catch (err) {
        console.error('Failed to create attempt:', err);
      }
    };

    const handleOpenChange = (open: boolean) => {
      if (!open) modal.hide();
    };

    useKeySubmitTask(handleCreate, {
      enabled: modal.visible && canCreate,
      scope: Scope.DIALOG,
      preventDefault: true,
    });

    return (
      <Dialog open={modal.visible} onOpenChange={handleOpenChange}>
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle>{t('createAttemptDialog.title')}</DialogTitle>
            <DialogDescription>
              {t('createAttemptDialog.description')}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            {profiles && (
              <div className="space-y-2">
                <ExecutorProfileSelector
                  profiles={profiles}
                  selectedProfile={effectiveProfile}
                  onProfileSelect={setUserSelectedProfile}
                  showLabel={true}
                />
              </div>
            )}

            <RepoBranchSelector
              configs={repoBranchConfigs}
              onBranchChange={setRepoBranch}
              isLoading={isLoadingBranches}
              className="space-y-2"
            />

            <div className="rounded-md border p-3 space-y-3">
              <div className="flex items-center justify-between gap-2">
                <div className="space-y-1">
                  <Label className="text-sm">Ralph mode</Label>
                  <p className="text-xs text-muted-foreground">
                    Run multiple autonomous iterations for this attempt.
                  </p>
                </div>
                <Switch
                  id="ralph-mode"
                  checked={ralphEnabled}
                  onCheckedChange={setRalphEnabled}
                  disabled={isCreating}
                  className="data-[state=checked]:bg-gray-900 dark:data-[state=checked]:bg-gray-100"
                />
              </div>
              {ralphEnabled && (
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1">
                    <Label className="text-xs text-muted-foreground">
                      Max iterations
                    </Label>
                    <Input
                      type="number"
                      min={1}
                      value={ralphMaxIterations}
                      onChange={(e) =>
                        setRalphMaxIterations(Number(e.target.value))
                      }
                    />
                  </div>
                  <div className="space-y-1">
                    <Label className="text-xs text-muted-foreground">
                      Max failures
                    </Label>
                    <Input
                      type="number"
                      min={1}
                      value={ralphMaxFailures}
                      onChange={(e) =>
                        setRalphMaxFailures(Number(e.target.value))
                      }
                    />
                  </div>
                </div>
              )}
            </div>

            {error && (
              <div className="text-sm text-destructive">
                {t('createAttemptDialog.error')}
              </div>
            )}
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => modal.hide()}
              disabled={isCreating}
            >
              {t('common:buttons.cancel')}
            </Button>
            <Button onClick={handleCreate} disabled={!canCreate}>
              {isCreating
                ? t('createAttemptDialog.creating')
                : t('createAttemptDialog.start')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const CreateAttemptDialog = defineModal<CreateAttemptDialogProps, void>(
  CreateAttemptDialogImpl
);
