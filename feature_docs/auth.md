# Auth System Overview

This project uses Google OAuth for SSO and a server-side session cookie. Access is
restricted by a per-project email whitelist, not by a global domain allowlist.

## Core flow

1. Frontend calls `GET /api/auth/google/start` to fetch `authorize_url`.
2. User signs in with Google, then Google redirects to
   `{VK_PUBLIC_BASE_URL}/api/auth/google/callback`.
3. Backend verifies Google tokens, upserts `auth_users`, creates an
   `auth_sessions` row, and sets the `vk_session` cookie.
4. Frontend checks `GET /api/auth/session` and renders the app or login screen.

## Sessions and cookies

- Cookie name: `vk_session`
- Storage: `auth_sessions` (SQLite)
- Expiry: 24 hours
- Cookie flags: `HttpOnly; SameSite=Lax; Secure` when `VK_PUBLIC_BASE_URL`
  is HTTPS

## Data model

- `auth_users`: user profile data keyed by email.
- `auth_sessions`: session rows linked to `auth_users`.
- `auth_oauth_states`: short-lived OAuth state for the login flow.
- `project_members`: membership whitelist (`project_id + email`) with role
  (`owner` or `member`).

## Authorization rules

- All protected API routes require `AuthenticatedUser` from session middleware.
- `load_project_middleware` enforces membership for project-scoped routes.
- Task and inbox routes additionally check membership by project id.
- Project list and WebSocket project stream are filtered by membership.
- Creating a project auto-adds the creator as `owner`.
- Membership changes broadcast a refreshed project snapshot to keep clients in sync.

## API endpoints

- `GET /api/auth/google/start` -> returns Google `authorize_url`.
- `GET /api/auth/google/callback` -> completes login and sets cookie.
- `GET /api/auth/session` -> returns `authenticated` + `user`.
- `GET /api/auth/logout` -> clears session.
- `GET /api/projects/:id/members` -> list project members.
- `POST /api/projects/:id/members` -> add member (owner only).
- `DELETE /api/projects/:id/members?email=...` -> remove member (owner only).

## Frontend integration

- `AuthGuard` uses `/api/auth/session` to gate access and show the login screen.
- `AuthLogin` starts the Google OAuth flow.
- `UserSystemInfo.current_user` is used for displaying the signed-in email and
  sign-out actions.

## Config and env vars

- `GOOGLE_CLIENT_ID` - OAuth client ID from Google Cloud Console
- `GOOGLE_CLIENT_SECRET` - OAuth client secret from Google Cloud Console
- `VK_PUBLIC_BASE_URL` - Public base URL (e.g. `https://vibe-kanban.example.com`);
  used to derive the OAuth redirect URI (`{VK_PUBLIC_BASE_URL}/api/auth/google/callback`)
  and to enable secure cookies over HTTPS

## Notes

- The OAuth `return_to` parameter only allows relative paths to prevent open
  redirects.
- `project_members` is separate from organization settings and is the only
  access control for projects in the local server.