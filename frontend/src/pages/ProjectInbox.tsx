import { useMemo, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { inboxApi } from '@/lib/api';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';
import type { InboxItemStatus } from 'shared/types';
import { Loader2, Check, X, Plus, Pencil, Clock } from 'lucide-react';

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

export function ProjectInbox() {
  const { projectId } = useParams();
  const dialogRef = useRef<HTMLDivElement>(null);
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [error, setError] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Inline editing state
  const [editingItemId, setEditingItemId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [editDescription, setEditDescription] = useState('');
  const [isSaving, setIsSaving] = useState(false);

  const resolvedProjectId = projectId ?? '';

  const pendingStatus: InboxItemStatus = 'pending';

  const {
    data: items,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ['inbox', resolvedProjectId],
    enabled: Boolean(resolvedProjectId),
    queryFn: () => inboxApi.list(resolvedProjectId, pendingStatus),
  });

  const pendingItems = useMemo(() => items ?? [], [items]);

  const handleCreate = async () => {
    if (!resolvedProjectId || !title.trim() || !body.trim()) return;
    setIsSubmitting(true);
    setError('');
    try {
      await inboxApi.create({
        project_id: resolvedProjectId,
        title: title.trim(),
        body: body.trim(),
        source_url: null,
      });
      setTitle('');
      setBody('');
      setIsDialogOpen(false);
      await refetch();
    } catch (err) {
      console.error('Failed to create inbox item', err);
      // @ts-expect-error ApiError message surface
      setError(err.message || 'Failed to create inbox item');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDialogClose = (open: boolean) => {
    if (!open) {
      setTitle('');
      setBody('');
      setError('');
    }
    setIsDialogOpen(open);
  };

  const handleAccept = async (id: string) => {
    setError('');
    try {
      const result = await inboxApi.accept(id);
      await refetch();
      // Show the CreateAttemptDialog to let user start work immediately or dismiss to leave in todo
      CreateAttemptDialog.show({ taskId: result.task_id });
    } catch (err) {
      console.error('Failed to accept inbox item', err);
      // @ts-expect-error ApiError message surface
      setError(err.message || 'Failed to accept inbox item');
    }
  };

  const handleDecline = async (id: string) => {
    setError('');
    try {
      await inboxApi.decline(id);
      await refetch();
    } catch (err) {
      console.error('Failed to decline inbox item', err);
      // @ts-expect-error ApiError message surface
      setError(err.message || 'Failed to decline inbox item');
    }
  };

  const handleStartEdit = (item: { id: string; title: string; prd_markdown: string | null }) => {
    setEditingItemId(item.id);
    setEditTitle(item.title);
    setEditDescription(item.prd_markdown || '');
  };

  const handleCancelEdit = () => {
    setEditingItemId(null);
    setEditTitle('');
    setEditDescription('');
  };

  const handleSaveEdit = async () => {
    if (!editingItemId) return;
    setIsSaving(true);
    setError('');
    try {
      await inboxApi.update(editingItemId, {
        title: editTitle.trim(),
        prd_markdown: editDescription.trim() || undefined,
      });
      setEditingItemId(null);
      setEditTitle('');
      setEditDescription('');
      await refetch();
    } catch (err) {
      console.error('Failed to update inbox item', err);
      // @ts-expect-error ApiError message surface
      setError(err.message || 'Failed to update inbox item');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="space-y-6 py-10 px-4">
      <div>
        <h1 className="text-2xl font-bold">Inbox</h1>
        <p className="text-sm text-muted-foreground">
          Review incoming items and accept or decline them.
        </p>
      </div>

      <div className="flex items-center gap-4">
        <Button onClick={() => setIsDialogOpen(true)}>
          <Plus className="mr-2 h-4 w-4" />
          Create inbox item
        </Button>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <Dialog ref={dialogRef} open={isDialogOpen} onOpenChange={handleDialogClose}>
        <DialogHeader>
          <DialogTitle>Create inbox item</DialogTitle>
          <DialogDescription>
            Add a manual item for this project.
          </DialogDescription>
        </DialogHeader>
        <DialogContent>
          <div className="space-y-2">
            <label className="text-sm font-medium">Title</label>
            <Input
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="Short summary"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Details</label>
            <Textarea
              value={body}
              onChange={(event) => setBody(event.target.value)}
              rows={12}
              placeholder="Describe the request or issue..."
            />
          </div>
        </DialogContent>
        <DialogFooter>
          <Button variant="outline" onClick={() => handleDialogClose(false)}>
            Cancel
          </Button>
          <Button
            onClick={handleCreate}
            disabled={isSubmitting || !title.trim() || !body.trim()}
          >
            {isSubmitting && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Create
          </Button>
        </DialogFooter>
      </Dialog>

      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">Pending items</h2>
          {isLoading && (
            <div className="flex items-center text-sm text-muted-foreground">
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              Loading
            </div>
          )}
        </div>

        {!isLoading && pendingItems.length === 0 && (
          <Card>
            <CardContent className="py-8 text-center text-sm text-muted-foreground">
              No pending items yet.
            </CardContent>
          </Card>
        )}

        {pendingItems.map((item) => {
          const isEditing = editingItemId === item.id;

          return (
            <Card key={item.id}>
              <CardHeader className="space-y-2">
                <div className="flex items-start justify-between gap-4">
                  {isEditing ? (
                    <Input
                      value={editTitle}
                      onChange={(e) => setEditTitle(e.target.value)}
                      className="text-lg font-semibold"
                      placeholder="Title"
                    />
                  ) : (
                    <CardTitle className="text-lg">{item.title}</CardTitle>
                  )}
                  <div className="flex items-center gap-2 shrink-0">
                    <span className="flex items-center gap-1 text-xs text-muted-foreground">
                      <Clock className="h-3 w-3" />
                      {formatTimeAgo(item.created_at)}
                    </span>
                    <Badge variant="secondary">{item.source}</Badge>
                    <Badge variant="outline">{item.kind}</Badge>
                  </div>
                </div>
                {item.source_url && (
                  <CardDescription>
                    <a
                      href={item.source_url}
                      target="_blank"
                      rel="noreferrer"
                      className="underline"
                    >
                      View source
                    </a>
                  </CardDescription>
                )}
              </CardHeader>
              <CardContent className="space-y-4">
                {isEditing ? (
                  <div className="space-y-2">
                    <h4 className="text-sm font-medium text-muted-foreground">
                      Description
                    </h4>
                    <Textarea
                      value={editDescription}
                      onChange={(e) => setEditDescription(e.target.value)}
                      rows={20}
                      placeholder="Describe the request or issue..."
                    />
                  </div>
                ) : (
                  item.prd_markdown && (
                    <div className="space-y-2">
                      <h4 className="text-sm font-medium text-muted-foreground">
                        Description
                      </h4>
                      <div className="whitespace-pre-wrap text-sm rounded-md bg-muted p-4">
                        {item.prd_markdown}
                      </div>
                    </div>
                  )
                )}
                <div className="flex gap-2 pt-2">
                  {isEditing ? (
                    <>
                      <Button onClick={handleSaveEdit} disabled={isSaving}>
                        {isSaving && (
                          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        )}
                        Save
                      </Button>
                      <Button
                        variant="outline"
                        onClick={handleCancelEdit}
                        disabled={isSaving}
                      >
                        Cancel
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button onClick={() => handleAccept(item.id)}>
                        <Check className="mr-2 h-4 w-4" />
                        Accept
                      </Button>
                      <Button
                        variant="outline"
                        onClick={() => handleDecline(item.id)}
                      >
                        <X className="mr-2 h-4 w-4" />
                        Decline
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => handleStartEdit(item)}
                      >
                        <Pencil className="h-4 w-4" />
                      </Button>
                    </>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
