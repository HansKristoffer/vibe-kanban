import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { InboxItem } from 'shared/types';
import { NewCardContent } from '../ui/new-card';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Textarea } from '../ui/textarea';
import { Badge } from '../ui/badge';
import { Checkbox } from '../ui/checkbox';
import { Label } from '../ui/label';
import { Alert, AlertDescription } from '../ui/alert';
import { Check, X, Pencil, Loader2, Clock } from 'lucide-react';
import WYSIWYGEditor from '@/components/ui/wysiwyg';
import {
  useAcceptInboxItem,
  useDeclineInboxItem,
  useUpdateInboxItem,
} from '@/hooks/useProjectInbox';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';

interface InboxItemPanelProps {
  item: InboxItem | null;
  onClose?: () => void;
}

function formatTimeAgo(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  const diffMs = Date.now() - d.getTime();
  const absSec = Math.round(Math.abs(diffMs) / 1000);

  if (absSec < 60) return 'just now';
  const mins = Math.round(absSec / 60);
  if (mins === 1) return '1 min ago';
  if (mins < 60) return `${mins} mins ago`;
  const hours = Math.round(mins / 60);
  if (hours === 1) return '1 hour ago';
  if (hours < 24) return `${hours} hours ago`;
  const days = Math.round(hours / 24);
  if (days === 1) return '1 day ago';
  if (days < 30) return `${days} days ago`;
  const months = Math.round(days / 30);
  if (months === 1) return '1 month ago';
  if (months < 12) return `${months} months ago`;
  const years = Math.round(months / 12);
  if (years === 1) return '1 year ago';
  return `${years} years ago`;
}

const InboxItemPanel = ({ item, onClose }: InboxItemPanelProps) => {
  const { t } = useTranslation('tasks');
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState('');
  const [editDescription, setEditDescription] = useState('');
  const [startTaskNow, setStartTaskNow] = useState(true);
  const [error, setError] = useState('');

  const acceptMutation = useAcceptInboxItem();
  const declineMutation = useDeclineInboxItem();
  const updateMutation = useUpdateInboxItem();

  const handleStartEdit = useCallback(() => {
    if (!item) return;
    setEditTitle(item.title);
    setEditDescription(item.prd_markdown || '');
    setIsEditing(true);
  }, [item]);

  const handleCancelEdit = useCallback(() => {
    setIsEditing(false);
    setEditTitle('');
    setEditDescription('');
  }, []);

  const handleSaveEdit = useCallback(async () => {
    if (!item) return;
    try {
      await updateMutation.mutateAsync({
        inboxId: item.id,
        payload: {
          title: editTitle.trim(),
          prd_markdown: editDescription.trim() || undefined,
        },
      });
      setIsEditing(false);
      setEditTitle('');
      setEditDescription('');
    } catch (err) {
      console.error('Failed to update inbox item', err);
    }
  }, [item, editTitle, editDescription, updateMutation]);

  const handleAccept = useCallback(async () => {
    if (!item) return;
    setError('');
    try {
      const result = await acceptMutation.mutateAsync(item.id);
      if (startTaskNow) {
        CreateAttemptDialog.show({ taskId: result.task_id });
      }
      onClose?.();
    } catch (err) {
      console.error('Failed to accept inbox item', err);
      setError(
        err instanceof Error ? err.message : 'Failed to accept inbox item'
      );
    }
  }, [item, acceptMutation, onClose, startTaskNow]);

  const handleDecline = useCallback(async () => {
    if (!item) return;
    try {
      await declineMutation.mutateAsync(item.id);
      onClose?.();
    } catch (err) {
      console.error('Failed to decline inbox item', err);
    }
  }, [item, declineMutation, onClose]);

  if (!item) {
    return (
      <div className="text-muted-foreground p-6">
        {t('inboxPanel.noItemSelected', 'No inbox item selected')}
      </div>
    );
  }

  const isLoading =
    acceptMutation.isPending ||
    declineMutation.isPending ||
    updateMutation.isPending;

  const titleContent = `# ${item.title || 'Inbox Item'}`;
  const descriptionContent = item.prd_markdown || '';

  return (
    <NewCardContent>
      <div className="p-6 flex flex-col h-full max-h-[calc(100vh-8rem)]">
        {/* Header with badges */}
        <div className="flex items-center justify-between gap-4 mb-4 shrink-0">
          <div className="flex items-center gap-2">
            <Badge variant="secondary">{item.source}</Badge>
            <Badge variant="outline">{item.kind}</Badge>
          </div>
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Clock className="h-4 w-4" />
            {formatTimeAgo(item.created_at)}
          </div>
        </div>

        {/* Source URL */}
        {item.source_url && (
          <div className="mb-4 shrink-0">
            <a
              href={item.source_url}
              target="_blank"
              rel="noreferrer"
              className="text-sm text-primary underline"
            >
              View source
            </a>
          </div>
        )}

        {/* Content */}
        <div className="space-y-3 overflow-y-auto flex-1 min-h-0">
          {isEditing ? (
            <div className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Title</label>
                <Input
                  value={editTitle}
                  onChange={(e) => setEditTitle(e.target.value)}
                  placeholder="Title"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Description</label>
                <Textarea
                  value={editDescription}
                  onChange={(e) => setEditDescription(e.target.value)}
                  rows={20}
                  placeholder="Describe the request or issue..."
                />
              </div>
            </div>
          ) : (
            <>
              <WYSIWYGEditor value={titleContent} disabled />
              {descriptionContent && (
                <WYSIWYGEditor value={descriptionContent} disabled />
              )}
            </>
          )}
        </div>

        {/* Error message */}
        {error && (
          <Alert variant="destructive" className="mt-4 shrink-0">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        {/* Actions */}
        <div className="pt-4 mt-4 border-t shrink-0">
          {isEditing ? (
            <div className="flex gap-2">
              <Button onClick={handleSaveEdit} disabled={isLoading}>
                {updateMutation.isPending && (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                )}
                {t('common:buttons.save', 'Save')}
              </Button>
              <Button
                variant="outline"
                onClick={handleCancelEdit}
                disabled={isLoading}
              >
                {t('common:buttons.cancel', 'Cancel')}
              </Button>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-2 mb-3">
                <Checkbox
                  id="start-task-now"
                  checked={startTaskNow}
                  onCheckedChange={(checked) => setStartTaskNow(checked === true)}
                />
                <Label htmlFor="start-task-now" className="text-sm cursor-pointer">
                  {t('inbox.startTaskNow', 'Start task now')}
                </Label>
              </div>
              <div className="flex gap-2">
                <Button onClick={handleAccept} disabled={isLoading}>
                  {acceptMutation.isPending && (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  )}
                  <Check className="mr-2 h-4 w-4" />
                  {t('inbox.accept', 'Accept')}
                </Button>
                <Button
                  variant="outline"
                  onClick={handleDecline}
                  disabled={isLoading}
                >
                  {declineMutation.isPending && (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  )}
                  <X className="mr-2 h-4 w-4" />
                  {t('inbox.decline', 'Decline')}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={handleStartEdit}
                  disabled={isLoading}
                >
                  <Pencil className="h-4 w-4" />
                </Button>
              </div>
            </>
          )}
        </div>
      </div>
    </NewCardContent>
  );
};

export default InboxItemPanel;
