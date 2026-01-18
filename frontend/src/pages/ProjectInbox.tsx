import { useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { inboxApi } from '@/lib/api';
import { InboxItemStatus } from 'shared/types';
import { Loader2, Check, X } from 'lucide-react';

export function ProjectInbox() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [error, setError] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const resolvedProjectId = projectId ?? '';

  const {
    data: items,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ['inbox', resolvedProjectId],
    enabled: Boolean(resolvedProjectId),
    queryFn: () => inboxApi.list(resolvedProjectId, InboxItemStatus.Pending),
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
      await refetch();
    } catch (err) {
      console.error('Failed to create inbox item', err);
      // @ts-expect-error ApiError message surface
      setError(err.message || 'Failed to create inbox item');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleAccept = async (id: string) => {
    setError('');
    try {
      const result = await inboxApi.accept(id);
      await refetch();
      navigate(`/projects/${resolvedProjectId}/tasks/${result.task_id}`);
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

  return (
    <div className="space-y-6 py-10 px-4">
      <div>
        <h1 className="text-2xl font-bold">Inbox</h1>
        <p className="text-sm text-muted-foreground">
          Review incoming items and accept or decline them.
        </p>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Create inbox item</CardTitle>
          <CardDescription>
            Add a manual item for this project.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
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
              rows={5}
              placeholder="Describe the request or issue..."
            />
          </div>
          <Button onClick={handleCreate} disabled={isSubmitting}>
            {isSubmitting && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Create inbox item
          </Button>
        </CardContent>
      </Card>

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

        {pendingItems.map((item) => (
          <Card key={item.id}>
            <CardHeader className="space-y-2">
              <CardTitle className="flex items-center justify-between">
                <span>{item.title}</span>
                <div className="flex items-center gap-2">
                  <Badge variant="secondary">{item.source}</Badge>
                  <Badge variant="outline">{item.kind}</Badge>
                </div>
              </CardTitle>
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
              {item.prd_markdown && (
                <pre className="whitespace-pre-wrap text-sm rounded-md bg-muted p-4">
                  {item.prd_markdown}
                </pre>
              )}
              <div className="flex gap-2">
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
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
