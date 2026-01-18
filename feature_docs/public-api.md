# Public API

This document describes the public API endpoints available for external integrations.

## Overview

The public API provides read-only access to project and task information without requiring authentication. These endpoints are designed for external services that need to query the current state of projects and their task pipelines.

**Base URL:** `/api/public`

## Endpoints

### GET /api/public/projects

Returns a list of all projects with their pipeline (task status) information.

#### Authentication

None required. This endpoint is completely open.

#### Response

```json
{
  "success": true,
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "My Project",
      "pipelines": [
        {
          "status": "todo",
          "task_count": 2,
          "tasks": [
            {
              "id": "660e8400-e29b-41d4-a716-446655440001",
              "title": "Implement feature X",
              "time_in_state_seconds": 86400,
              "state_since": "2026-01-17T10:00:00Z"
            },
            {
              "id": "660e8400-e29b-41d4-a716-446655440002",
              "title": "Fix bug Y",
              "time_in_state_seconds": 3600,
              "state_since": "2026-01-18T09:00:00Z"
            }
          ]
        },
        {
          "status": "inprogress",
          "task_count": 1,
          "tasks": [
            {
              "id": "660e8400-e29b-41d4-a716-446655440003",
              "title": "Working on feature Z",
              "time_in_state_seconds": 7200,
              "state_since": "2026-01-18T08:00:00Z"
            }
          ]
        },
        {
          "status": "inreview",
          "task_count": 0,
          "tasks": []
        },
        {
          "status": "done",
          "task_count": 5,
          "tasks": [...]
        },
        {
          "status": "cancelled",
          "task_count": 0,
          "tasks": []
        }
      ]
    }
  ]
}
```

#### Data Types

##### Project

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Unique project identifier |
| `name` | string | Project name |
| `pipelines` | Pipeline[] | Array of pipeline statuses with their tasks |

##### Pipeline

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Pipeline status: `todo`, `inprogress`, `inreview`, `done`, or `cancelled` |
| `task_count` | number | Number of tasks in this pipeline |
| `tasks` | Task[] | Array of tasks in this pipeline |

##### Task

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Unique task identifier |
| `title` | string | Task title |
| `time_in_state_seconds` | number | How long the task has been in the current state (in seconds) |
| `state_since` | ISO 8601 date | Timestamp when the task entered its current state |

#### Example Usage

```bash
# Fetch all projects with pipeline status
curl http://localhost:8080/api/public/projects
```

```javascript
// JavaScript/TypeScript
const response = await fetch('http://localhost:8080/api/public/projects');
const data = await response.json();

if (data.success) {
  for (const project of data.data) {
    console.log(`Project: ${project.name}`);
    for (const pipeline of project.pipelines) {
      console.log(`  ${pipeline.status}: ${pipeline.task_count} tasks`);
    }
  }
}
```

```python
# Python
import requests

response = requests.get('http://localhost:8080/api/public/projects')
data = response.json()

if data['success']:
    for project in data['data']:
        print(f"Project: {project['name']}")
        for pipeline in project['pipelines']:
            print(f"  {pipeline['status']}: {pipeline['task_count']} tasks")
```

#### Error Responses

| Status Code | Description |
|-------------|-------------|
| 200 | Success |
| 500 | Internal server error (database connection issue) |

## Implementation Details

### Source Files

- **Route handler:** `crates/server/src/routes/public.rs`
- **Route registration:** `crates/server/src/routes/mod.rs`
- **Task query:** `crates/db/src/models/task.rs` (`find_by_project_id` method)

### Notes

- The `time_in_state_seconds` is calculated based on the task's `updated_at` timestamp, which represents the last time the task was modified (including status changes).
- All pipelines are always returned, even if they have zero tasks.
- Tasks within each pipeline are ordered by `updated_at` descending (most recently updated first).
