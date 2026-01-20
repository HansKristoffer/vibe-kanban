import { useUserSystem } from '../../components/ConfigProvider';

export function useAuth() {
  const { loginStatus, loading, system } = useUserSystem();
  const currentUser = system.current_user;

  return {
    isSignedIn: Boolean(currentUser),
    isLoaded: !loading && loginStatus !== null,
    userId:
      currentUser?.email ??
      (loginStatus?.status === 'loggedin' ? loginStatus.profile.user_id : null),
  };
}
