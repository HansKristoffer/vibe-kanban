import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Label } from '@radix-ui/react-label';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useCallback, useState } from 'react';
import { attemptsApi } from '@/lib/api.ts';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { Alert } from '@/components/ui/alert';
import { defineModal } from '@/lib/modals';

interface CommitDialogProps {
  workspaceId: string;
  repoId: string;
}

export type CommitDialogResult = {
  success: boolean;
  error?: string;
};

const CommitDialogImpl = NiceModal.create<CommitDialogProps>(
  ({ workspaceId, repoId }) => {
    const modal = useModal();
    const { t } = useTranslation('tasks');
    const [commitMessage, setCommitMessage] = useState('');
    const [isCommitting, setIsCommitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleCommit = useCallback(async () => {
      if (!workspaceId || !repoId || !commitMessage.trim()) return;

      setIsCommitting(true);
      setError(null);

      const result = await attemptsApi.commit(workspaceId, {
        message: commitMessage,
        repo_id: repoId,
      });

      setIsCommitting(false);

      if (!result.success) {
        setError(
          result.message || t('commitDialog.errors.commitFailed')
        );
        return;
      }

      setCommitMessage('');
      modal.resolve({ success: true } as CommitDialogResult);
      modal.hide();
    }, [workspaceId, repoId, commitMessage, modal, t]);

    const handleCancel = useCallback(() => {
      const result: CommitDialogResult = error
        ? { success: false, error }
        : { success: false };
      modal.resolve(result);
      modal.hide();
      setCommitMessage('');
      setError(null);
    }, [modal, error]);

    return (
      <Dialog open={modal.visible} onOpenChange={() => handleCancel()}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>{t('commitDialog.title')}</DialogTitle>
            <DialogDescription>
              {t('commitDialog.description')}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="commit-message">
                {t('commitDialog.messageLabel')}
              </Label>
              <Input
                id="commit-message"
                value={commitMessage}
                onChange={(e) => setCommitMessage(e.target.value)}
                placeholder={t('commitDialog.messagePlaceholder')}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && commitMessage.trim()) {
                    handleCommit();
                  }
                }}
                autoFocus
              />
            </div>
            {error && <Alert variant="destructive">{error}</Alert>}
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={handleCancel}
              disabled={isCommitting}
            >
              {t('common:buttons.cancel')}
            </Button>
            <Button
              onClick={handleCommit}
              disabled={isCommitting || !commitMessage.trim()}
              className="bg-blue-600 hover:bg-blue-700"
            >
              {isCommitting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t('commitDialog.committing')}
                </>
              ) : (
                t('commitDialog.commitButton')
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const CommitDialog = defineModal<CommitDialogProps, CommitDialogResult>(
  CommitDialogImpl
);
