import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { authApi } from '@/lib/api';

interface AuthLoginProps {
  returnTo?: string;
}

export function AuthLogin({ returnTo }: AuthLoginProps) {
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const handleSignIn = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await authApi.startGoogleLogin(returnTo);
      window.location.assign(response.authorize_url);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to start sign-in';
      setError(message);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="legacy-design flex min-h-screen flex-col items-center justify-center bg-background">
      <div className="flex flex-col items-center gap-6 rounded-lg border border-border bg-card p-8 shadow-lg">
        <div className="flex flex-col items-center gap-2">
          <h1 className="text-2xl font-semibold text-foreground">
            Sign in to continue
          </h1>
          <p className="text-sm text-muted-foreground">
            Google SSO is required to access this application
          </p>
        </div>
        {error && (
          <div className="w-full rounded-md bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </div>
        )}
        <Button onClick={handleSignIn} disabled={isLoading}>
          {isLoading ? 'Redirecting...' : 'Sign in with Google'}
        </Button>
      </div>
    </div>
  );
}
