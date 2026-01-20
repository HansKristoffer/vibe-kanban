import { useQuery } from '@tanstack/react-query';
import { authApi, type AuthSessionResponse } from '@/lib/api';

export function useAuthSession() {
  return useQuery<AuthSessionResponse>({
    queryKey: ['auth', 'session'],
    queryFn: () => authApi.session(),
    staleTime: 60 * 1000,
    retry: false,
  });
}
