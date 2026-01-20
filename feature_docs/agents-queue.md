# Global agent queue (FIFO)

This document describes the global AI agent queue that limits concurrent coding
agent runs to 10 across all projects. It provides context for future
maintenance and extension.

## Why it exists

We want to prevent overload by ensuring only a fixed number of coding agents run
at once. Additional start requests should wait, preserve FIFO ordering, and
start automatically as capacity frees.

## What is queued

Only `execution_processes` with `run_reason = 'codingagent'` count toward the
limit. Setup/cleanup scripts and dev servers do not count.

## Key behavior

- Global limit: max 10 concurrent `codingagent` processes.
- FIFO queue: extra starts become `status = 'queued'` and are started oldest
  first.
- Tasks stay in the **In Progress** pipeline while queued.
- The UI shows a waiting indicator on task cards and `queued` status in process
  lists.

## Core implementation points

### Start path (enqueue)

File: `crates/services/src/services/container.rs`

Function: `start_execution`

When `run_reason == ExecutionProcessRunReason::CodingAgent`:

1. Count running coding agents.
2. If `running >= 10` OR any queued exists, create the execution process with
   `status = queued` and return without spawning.
3. Otherwise create the process as `running` and spawn immediately.

This preserves FIFO by preventing newly created requests from bypassing earlier
queued items.

### Drainer (dequeue)

File: `crates/local-deployment/src/container.rs`

Function: `drain_queued_coding_agents`

The drainer:

- Computes available slots: `10 - running_count`.
- Selects the oldest queued coding-agent processes.
- Claims each row atomically by changing `status` from `queued` → `running`.
- Starts the executor process and log normalization.
- Marks the process `failed` if startup fails.

The drainer runs:

- Periodically (every ~5s) via `spawn_queued_execution_drainer`.
- After any execution completes in the exit monitor.

### Task-level queued indicator

File: `crates/db/src/models/task.rs`

Query: `Task::find_by_project_id_with_attempt_status`

Adds `has_queued_attempt` (true if any queued coding-agent process exists for the
task’s workspaces). The UI uses this to show “Waiting for available agent slot.”

### UI display

- Task card indicator: `frontend/src/components/tasks/TaskCard.tsx`
- Process list status: `frontend/src/components/ui-new/primitives/ProcessListItem.tsx`
- Process details tab: `frontend/src/components/tasks/TaskDetails/ProcessesTab.tsx`

## Data model notes

- `ExecutionProcessStatus` includes `queued`.
- Migration added to allow `status IN ('queued','running','completed','failed','killed')`.

## Operational notes

- If you change the limit, update both the enqueue check and the drainer.
- If you add other run reasons that should count toward the limit, update the
  running-count query accordingly.
