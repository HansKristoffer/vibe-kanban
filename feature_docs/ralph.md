Ralph mode (single task attempt) setup

Overview
- Opt-in automation that loops a single task attempt/workspace.
- Runs the coding agent multiple iterations until it either:
  - emits <promise>COMPLETE</promise>, or
  - hits max iterations, or
  - exceeds max consecutive failures.
- Each iteration starts with **fresh agent context** (no session reuse).
- The PRD (checklist) is sourced from **Task.description**, not from filesystem files.
- When Ralph stops/completes, the task transitions to In Review like any other attempt.

How it works
1. Task description MUST contain a Markdown checklist (lines starting with `- [ ]`).
2. Each iteration picks EXACTLY ONE unchecked item and implements only that.
3. The agent creates/updates `progress.txt` to track completed items across runs.
4. When all items are done, the agent outputs `<promise>COMPLETE</promise>`.
5. Commits happen automatically after each successful iteration.

Example task description:
```
Build a user authentication system.

- [ ] Create login page with email/password form
- [ ] Implement JWT token generation
- [ ] Add password reset flow
- [ ] Write unit tests for auth service
```

Persistence layer
- Storage is in SQLite table workspace_automations (see migration
  crates/db/migrations/20260118000000_add_workspace_automations.sql).
- Data model lives in crates/db/src/models/workspace_automation.rs.
- Model module is registered in crates/db/src/models/mod.rs.
- Key fields:
  - workspace_id (PK), mode, status
  - iteration, max_iterations
  - consecutive_failures, max_failures
  - last_error

How it is created (opt-in)
- Ralph is off by default.
- When a task attempt is created with ralph config:
  - The backend inserts a workspace_automations row with:
    - status = running
    - iteration = 1
    - max_iterations / max_failures from request (defaults 10 / 3)
    - consecutive_failures = 0
- Entry points:
  - POST /api/task-attempts (CreateTaskAttemptBody in
    crates/server/src/routes/task_attempts.rs)
  - POST /api/projects/:id/tasks/create-and-start (CreateAndStartTaskRequest in
    crates/server/src/routes/tasks.rs)
- MCP task server also uses CreateTaskAttemptBody and sets ralph: null in
  crates/server/src/mcp/task_server.rs.

Runtime loop
- The loop is implemented in the execution finalization path in
  crates/local-deployment/src/container.rs.
- After each execution completes, the service checks:
  - Is there a workspace_automations row for this workspace?
  - Is status == running?
- If yes, Ralph decides next step:
  - If last coding agent summary contains <promise>COMPLETE</promise>
    -> mark automation completed and finalize.
  - If consecutive_failures > max_failures
    -> mark automation stopped and finalize.
  - If iteration >= max_iterations
    -> mark automation stopped and finalize.
  - Otherwise:
    - Build the next Ralph prompt (with fresh context).
    - Increment iteration.
    - Start a NEW initial execution (not a follow-up).
    - Ignore any queued manual follow-up while Ralph is running.

Prompt composition
- Built by build_ralph_prompt() and assemble_ralph_prompt() in
  crates/local-deployment/src/container.rs.
- Includes:
  - task title + description (Task::to_prompt) — this IS the PRD/checklist
  - progress.txt content from workspace root or agent working dir
  - last error excerpt (stderr tail) on failure
  - instructions enforcing one-item-per-run semantics

Key files for durable state
- `progress.txt`: Created by the agent to track which checklist items are done.
  Read at the start of each iteration to provide context. Located in workspace
  root or agent working dir.

Failure handling
- Any failed/killed execution increments consecutive_failures and stores last_error.
- A successful execution resets consecutive_failures to 0.
- Once failures exceed max_failures, Ralph stops and task finalizes to In Review.

UI / API control
- UI toggles live in:
  - CreateAttemptDialog (frontend/src/components/dialogs/tasks/CreateAttemptDialog.tsx)
  - TaskFormDialog (frontend/src/components/dialogs/tasks/TaskFormDialog.tsx)
- Status + stop control displayed in TaskFollowUpSection:
  frontend/src/components/tasks/TaskFollowUpSection.tsx
- API endpoints defined in:
  crates/server/src/routes/task_attempts.rs
  - GET /api/task-attempts/:workspace_id/ralph
  - POST /api/task-attempts/:workspace_id/ralph/start
  - POST /api/task-attempts/:workspace_id/ralph/stop
- Frontend API client wiring in:
  frontend/src/lib/api.ts
- Attempt creation mutation wiring in:
  frontend/src/hooks/useAttemptCreation.ts

Notes for future development
- Ralph is scoped to a single task attempt/workspace.
- There is no auto-resume on server restart yet.
- PRD.md files are no longer read; the checklist must be in Task.description.
