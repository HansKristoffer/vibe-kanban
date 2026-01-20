PRAGMA foreign_keys = OFF;

-- sqlx workaround due to lack of `-- no-transaction` in sqlx-sqlite.
COMMIT TRANSACTION;

BEGIN TRANSACTION;

CREATE TABLE execution_processes_new (
    id              BLOB PRIMARY KEY,
    session_id      BLOB NOT NULL,
    run_reason      TEXT NOT NULL DEFAULT 'setupscript'
                       CHECK (run_reason IN ('setupscript','codingagent','devserver','cleanupscript')),
    executor_action TEXT NOT NULL DEFAULT '{}',
    status          TEXT NOT NULL DEFAULT 'running'
                       CHECK (status IN ('queued','running','completed','failed','killed')),
    exit_code       INTEGER,
    dropped         INTEGER NOT NULL DEFAULT 0,
    started_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    completed_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

INSERT INTO execution_processes_new (
    id,
    session_id,
    run_reason,
    executor_action,
    status,
    exit_code,
    dropped,
    started_at,
    completed_at,
    created_at,
    updated_at
)
SELECT
    id,
    session_id,
    run_reason,
    executor_action,
    status,
    exit_code,
    dropped,
    started_at,
    completed_at,
    created_at,
    updated_at
FROM execution_processes;

DROP TABLE execution_processes;
ALTER TABLE execution_processes_new RENAME TO execution_processes;

CREATE INDEX IF NOT EXISTS idx_execution_processes_session_id
    ON execution_processes(session_id);
CREATE INDEX IF NOT EXISTS idx_execution_processes_status
    ON execution_processes(status);
CREATE INDEX IF NOT EXISTS idx_execution_processes_run_reason
    ON execution_processes(run_reason);
CREATE INDEX IF NOT EXISTS idx_execution_processes_session_status_run_reason
    ON execution_processes (session_id, status, run_reason);
CREATE INDEX IF NOT EXISTS idx_execution_processes_session_run_reason_created
    ON execution_processes (session_id, run_reason, created_at DESC);

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;

-- sqlx workaround due to lack of `-- no-transaction` in sqlx-sqlite.
BEGIN TRANSACTION;
