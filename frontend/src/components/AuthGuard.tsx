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
  const { data: session, isLoading } = useAuthSession();

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
