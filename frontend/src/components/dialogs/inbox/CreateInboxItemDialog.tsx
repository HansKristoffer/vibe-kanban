import { useState, useEffect } from 'react';
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
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Checkbox } from '@/components/ui/checkbox';
import { Loader2 } from 'lucide-react';
import { useCreateInboxItem } from '@/hooks/useProjectInbox';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';
import { useKeySubmitTask, Scope } from '@/keyboard';

export interface CreateInboxItemDialogProps {
  projectId: string;
}

const CreateInboxItemDialogImpl = NiceModal.create<CreateInboxItemDialogProps>(
  ({ projectId }) => {
    const modal = useModal();
    const { t } = useTranslation('tasks');
    const [title, setTitle] = useState('');
    const [body, setBody] = useState('');
    const [generatePrd, setGeneratePrd] = useState(true);
    const [error, setError] = useState('');

    const createMutation = useCreateInboxItem();

    useEffect(() => {
      if (!modal.visible) {
        setTitle('');
        setBody('');
        setGeneratePrd(true);
        setError('');
      }
    }, [modal.visible]);

    const canCreate = title.trim() && body.trim() && !createMutation.isPending;

    const handleCreate = async () => {
      if (!canCreate) return;
      setError('');
      try {
        await createMutation.mutateAsync({
          project_id: projectId,
          title: title.trim(),
          body: body.trim(),
          generate_prd: generatePrd,
        });
        modal.resolve(generatePrd); // Pass whether PRD generation was requested
        modal.hide();
      } catch (err) {
        console.error('Failed to create inbox item', err);
        setError(
          err instanceof Error ? err.message : 'Failed to create inbox item'
        );
      }
    };

    const handleOpenChange = (open: boolean) => {
      if (!open) {
        modal.reject();
        modal.hide();
      }
    };

    useKeySubmitTask(handleCreate, {
      enabled: modal.visible && Boolean(canCreate),
      scope: Scope.DIALOG,
      preventDefault: true,
    });

    return (
      <Dialog open={modal.visible} onOpenChange={handleOpenChange}>
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle>
              {t('inbox.createDialog.title', 'Create inbox item')}
            </DialogTitle>
            <DialogDescription>
              {t(
                'inbox.createDialog.description',
                'Add a new item to the inbox for this project.'
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="inbox-title">
                {t('inbox.createDialog.titleLabel', 'Title')}
              </Label>
              <Input
                id="inbox-title"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder={t(
                  'inbox.createDialog.titlePlaceholder',
                  'Short summary'
                )}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="inbox-body">
                {t('inbox.createDialog.detailsLabel', 'Details')}
              </Label>
              <Textarea
                id="inbox-body"
                value={body}
                onChange={(e) => setBody(e.target.value)}
                rows={12}
                placeholder={t(
                  'inbox.createDialog.detailsPlaceholder',
                  'Describe the request or issue...'
                )}
              />
            </div>

            <div className="flex items-start space-x-3">
              <Checkbox
                id="generate-prd"
                checked={generatePrd}
                onCheckedChange={(checked) => setGeneratePrd(checked === true)}
              />
              <div className="grid gap-1.5 leading-none">
                <Label
                  htmlFor="generate-prd"
                  className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
                >
                  {t(
                    'inbox.createDialog.generatePrdLabel',
                    'Generate PRD from description'
                  )}
                </Label>
                <p className="text-xs text-muted-foreground">
                  {t(
                    'inbox.createDialog.generatePrdDescription',
                    'Uses AI to create a structured PRD from your description. Generates in the background after creation.'
                  )}
                </p>
              </div>
            </div>

            {error && (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => handleOpenChange(false)}
              disabled={createMutation.isPending}
            >
              {t('common:buttons.cancel', 'Cancel')}
            </Button>
            <Button onClick={handleCreate} disabled={!canCreate}>
              {createMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              {t('inbox.createDialog.create', 'Create')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const CreateInboxItemDialog = defineModal<
  CreateInboxItemDialogProps,
  boolean // Returns true if PRD generation was requested (happens in background)
>(CreateInboxItemDialogImpl);
