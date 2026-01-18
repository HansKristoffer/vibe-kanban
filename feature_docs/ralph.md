Ralph mode (single task attempt) setup

Overview
- Opt-in automation that loops a single task attempt/workspace.
- Runs the coding agent multiple iterations until it either:
  - emits <promise>COMPLETE</promise>, or
  - hits max iterations, or
  - exceeds max consecutive failures.
- When Ralph stops/completes, the task transitions to In Review like any other attempt.

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

Runtime loop (how it works)
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
    - Build the next Ralph prompt.
    - Increment iteration.
    - Start a follow-up execution in the same session/workspace.
    - Ignore any queued manual follow-up while Ralph is running.

Prompt composition
- Built by build_ralph_prompt() and assemble_ralph_prompt() in
  crates/local-deployment/src/container.rs.
- Includes:
  - task title + description (Task::to_prompt)
  - optional PRD.md and progress.txt from workspace root or agent working dir
  - last error excerpt (stderr tail) on failure
  - standard instructions (small step, update progress, emit COMPLETE)

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
- PRD/progress files are optional; Ralph will run without them.
