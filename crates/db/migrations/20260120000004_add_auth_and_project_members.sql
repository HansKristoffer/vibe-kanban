PRAGMA foreign_keys = ON;

CREATE TABLE auth_users (
    id          BLOB PRIMARY KEY,
    email       TEXT NOT NULL UNIQUE,
    name        TEXT,
    picture_url TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

CREATE TABLE auth_sessions (
    id         TEXT PRIMARY KEY,
    user_id    BLOB NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (user_id) REFERENCES auth_users(id) ON DELETE CASCADE
);
CREATE INDEX idx_auth_sessions_user_id ON auth_sessions(user_id);
CREATE INDEX idx_auth_sessions_expires_at ON auth_sessions(expires_at);

CREATE TABLE auth_oauth_states (
    id         TEXT PRIMARY KEY,
    return_to  TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);
CREATE INDEX idx_auth_oauth_states_expires_at ON auth_oauth_states(expires_at);

CREATE TABLE project_members (
    project_id BLOB NOT NULL,
    email      TEXT NOT NULL,
    role       TEXT NOT NULL DEFAULT 'member'
               CHECK (role IN ('owner', 'member')),
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    PRIMARY KEY (project_id, email),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX idx_project_members_project_id ON project_members(project_id);
CREATE INDEX idx_project_members_email ON project_members(email);
