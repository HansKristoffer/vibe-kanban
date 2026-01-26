import { useQuery } from '@tanstack/react-query';
import { attemptsApi, type WorkspaceAssetsResponse } from '@/lib/api';

export const workspaceAssetsKeys = {
  all: ['workspaceAssets'] as const,
  byWorkspace: (workspaceId: string) =>
    [...workspaceAssetsKeys.all, workspaceId] as const,
};

export interface UseWorkspaceAssetsOptions {
  enabled?: boolean;
  /**
   * Poll interval in milliseconds. Set to false to disable polling.
   * Default: 5000 (5 seconds)
   */
  refetchInterval?: number | false;
}

export interface UseWorkspaceAssetsResult {
  assets: WorkspaceAssetsResponse['assets'];
  total: number;
  isLoading: boolean;
  error: Error | null;
  refetch: () => void;
}

/**
 * Hook for fetching workspace assets (screenshots and videos).
 * Polls for updates while the workspace is active.
 */
export function useWorkspaceAssets(
  workspaceId: string | undefined,
  options: UseWorkspaceAssetsOptions = {}
): UseWorkspaceAssetsResult {
  const { enabled = true, refetchInterval = 5000 } = options;

  const query = useQuery<WorkspaceAssetsResponse, Error>({
    queryKey: workspaceAssetsKeys.byWorkspace(workspaceId ?? ''),
    queryFn: () => attemptsApi.getAssets(workspaceId!),
    enabled: enabled && !!workspaceId,
    refetchInterval,
    staleTime: 2000, // Consider data stale after 2 seconds
  });

  return {
    assets: query.data?.assets ?? [],
    total: query.data?.total ?? 0,
    isLoading: query.isLoading,
    error: query.error,
    refetch: query.refetch,
  };
}
