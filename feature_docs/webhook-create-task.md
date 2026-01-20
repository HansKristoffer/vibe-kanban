# Personal AI Quick Task Webhook

This webhook allows your personal AI (or any external system) to quickly create and start a task in Vibe Kanban. It combines PRD generation, Slack notification, and agent execution into a single API call.

## Endpoint

```
POST /api/webhooks/personal-ai/{webhook_token}
```

### Authentication

The endpoint uses the project's existing `webhook_token` for authentication. You can find this token in your project's integration settings.

## Request

### Headers

```
Content-Type: application/json
```

### Body

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `text` | string | Yes | The idea or task description from your personal AI |
| `title` | string | No | Override the auto-generated title |
| `source_item_id` | string | No | Idempotency key (defaults to a random UUID) |
| `source_url` | string | No | Link to the source/context of the idea |
| `base_branch` | string | No | Base branch for the workspace (default: `"main"`) |
| `slack_user_id` | string | No | Slack user ID to mention in notifications (recommended - see notes) |

### Example Request

```bash
curl -X POST "https://your-vk-instance/api/webhooks/personal-ai/your-webhook-token" \
  -H "Content-Type: application/json" \
  -d '{
    "text": "Add a dark mode toggle to the settings page. Should persist the preference in localStorage and apply a dark theme class to the body.",
    "title": "Add dark mode toggle",
    "base_branch": "main",
    "slack_user_id": "U1234567890"
  }'
```

## Response

### Success Response (200 OK)

```json
{
  "success": true,
  "data": {
    "inbox_item_id": "550e8400-e29b-41d4-a716-446655440000",
    "task_id": "550e8400-e29b-41d4-a716-446655440001",
    "task_url": "https://your-vk-instance/projects/project-uuid/tasks/550e8400-e29b-41d4-a716-446655440001",
    "workspace_id": "550e8400-e29b-41d4-a716-446655440002",
    "execution_process_id": "550e8400-e29b-41d4-a716-446655440003",
    "slack_posted": true,
    "slack_channel_id": "C1234567890",
    "slack_message_ts": "1234567890.123456",
    "started": true,
    "start_error": null
  }
}
```

### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `inbox_item_id` | UUID | The created inbox item ID |
| `task_id` | UUID | The created task ID |
| `task_url` | string | URL to view the task in Vibe Kanban (requires `VK_PUBLIC_BASE_URL` env var for absolute URL) |
| `workspace_id` | UUID | The created workspace ID (null if no repos configured) |
| `execution_process_id` | UUID | The execution process ID (null if agent didn't start) |
| `slack_posted` | boolean | Whether the Slack message was posted successfully |
| `slack_channel_id` | string | The Slack channel where the message was posted |
| `slack_message_ts` | string | The Slack message timestamp |
| `started` | boolean | Whether the Claude Code agent started successfully |
| `start_error` | string | Error message if the agent failed to start |

### Error Response (400/500)

```json
{
  "success": false,
  "error": "Unknown webhook token"
}
```

## What Happens

When you call this endpoint, the following occurs in sequence:

1. **PRD Generation**: The `text` is sent to Anthropic to generate a structured PRD (Product Requirements Document). If LLM generation fails, the raw text is used as the PRD.

2. **Inbox Item Creation**: An inbox item is created with the generated PRD.

3. **Slack Notification** (best-effort): If Slack is configured for the project, a message is posted to the configured channel showing the PRD with an "Accepted" status (no action buttons since it's auto-accepted).

4. **Task Creation**: A task is created with the PRD as the description.

5. **Linear Issue** (best-effort): If Linear is configured, an issue is created and linked to the task.

6. **Agent Execution**: A workspace is created and Claude Code is started immediately using the DEFAULT profile (which skips permission prompts).

## Use Cases

- **Personal AI Integration**: Have your personal AI assistant quickly queue up implementation tasks
- **Automated Task Creation**: Create tasks from external triggers (CI/CD, monitoring alerts, etc.)
- **Quick Ideas**: Capture ideas that need immediate action without manual approval steps

## Notes

- The agent starts with Claude Code's DEFAULT profile, which has `dangerously_skip_permissions: true` - this means the agent will execute without requiring approval for each action.
- Slack posting is best-effort; the task will still be created and the agent will start even if Slack integration fails.
- The endpoint requires at least one repository configured for the project to start the agent.
- **Recommended: Provide `slack_user_id`** - If you don't provide a `slack_user_id`, subsequent Slack notifications (like "task needs review" or "task failed") will still be posted but won't tag anyone. To ensure you get notified when the agent needs attention, include your Slack user ID in the request.
