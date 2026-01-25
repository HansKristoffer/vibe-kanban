import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { inboxApi } from '@/lib/api';
import type { InboxItemStatus } from 'shared/types';

const INBOX_QUERY_KEY = 'inbox';

export function useProjectInbox(
  projectId: string | undefined,
  status: InboxItemStatus = 'pending'
) {
  return useQuery({
    queryKey: [INBOX_QUERY_KEY, projectId, status],
    queryFn: async () => {
      if (!projectId) return [];
      const items = await inboxApi.list(projectId, status);
      // Sort by created_at descending (newest first)
      return items.sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    },
    enabled: Boolean(projectId),
  });
}

export function useInboxItem(inboxId: string | undefined) {
  return useQuery({
    queryKey: [INBOX_QUERY_KEY, 'item', inboxId],
    queryFn: async () => {
      if (!inboxId) return null;
      return inboxApi.get(inboxId);
    },
    enabled: Boolean(inboxId),
  });
}

export function useAcceptInboxItem() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (inboxId: string) => {
      return inboxApi.accept(inboxId);
    },
    onSuccess: () => {
      // Invalidate inbox queries to refresh the list
      queryClient.invalidateQueries({ queryKey: [INBOX_QUERY_KEY] });
    },
  });
}

export function useDeclineInboxItem() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (inboxId: string) => {
      return inboxApi.decline(inboxId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [INBOX_QUERY_KEY] });
    },
  });
}

export function useCreateInboxItem() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (payload: {
      project_id: string;
      title: string;
      body: string;
      source_url?: string | null;
      generate_prd?: boolean;
    }) => {
      return inboxApi.create({
        project_id: payload.project_id,
        title: payload.title,
        body: payload.body,
        source_url: payload.source_url ?? null,
        generate_prd: payload.generate_prd ?? true,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [INBOX_QUERY_KEY] });
    },
  });
}

export function useUpdateInboxItem() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      inboxId,
      payload,
    }: {
      inboxId: string;
      payload: { title?: string; prd_markdown?: string };
    }) => {
      return inboxApi.update(inboxId, payload);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [INBOX_QUERY_KEY] });
    },
  });
}
