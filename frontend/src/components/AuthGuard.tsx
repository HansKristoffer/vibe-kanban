import { ReactNode } from 'react';
import { useAuthSession } from '@/hooks/auth/useAuthSession';
import { AuthLogin } from '@/components/AuthLogin';
import { Loader } from '@/components/ui/loader';
// Import CSS for auth pages that render outside design scopes
import '@/styles/legacy/index.css';

interface AuthGuardProps {
  children: ReactNode;
}

export function AuthGuard({ children }: AuthGuardProps) {
  // Check if auth is disabled (set via vite config from env)
  const authDisabled = import.meta.env.VITE_AUTH_DISABLED === 'true';

  const { data: session, isLoading } = useAuthSession();

  // Bypass authentication when auth is disabled
  if (authDisabled) {
    return <>{children}</>;
  }

  if (isLoading) {
    return (
      <div className="legacy-design flex min-h-screen items-center justify-center bg-background">
        <Loader message="Checking authentication..." size={32} />
      </div>
    );
  }

  if (!session?.authenticated) {
    return (
      <AuthLogin returnTo={`${window.location.pathname}${window.location.search}`} />
    );
  }

  return <>{children}</>;
}
