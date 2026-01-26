# Task Assets Feature

## Overview

Task assets allow AI agents to capture screenshots and video recordings to document UI changes. These assets can be:
- Viewed in the workspace UI
- Selected and included in Pull Request descriptions
- Managed (listed, deleted, cleared) by the AI agent

## How It Works

1. **Capture**: The AI uses the `vibe_recorder` MCP tool to capture screenshots or record videos.
2. **Storage**: Assets are stored in `.vibe-assets/` directory within the workspace worktree.
3. **Display**: The frontend displays assets in a collapsible "Assets" section in the right sidebar.
4. **PR Integration**: When creating a PR, users can select assets to include in the description.

## Storage Structure

Assets are stored in the workspace worktree:

```
.vibe-assets/
  manifest.json        # Metadata for all assets
  {uuid}.png           # Screenshots
  {uuid}.mp4           # Video recordings
  .gitignore           # Prevents assets from being committed
```

### Manifest Format

```json
{
  "version": 1,
  "assets": [
    {
      "id": "abc123-...",
      "asset_type": "screenshot",
      "filename": "abc123.png",
      "description": "Login page after styling changes",
      "related_files": ["src/components/Login.tsx"],
      "captured_at": "2026-01-25T12:00:00Z",
      "size_bytes": 125000
    }
  ]
}
```

## MCP Tool: vibe_recorder

The `vibe_recorder` MCP server provides the following tools for AI agents:

### screenshot
Capture a screenshot of the current browser state.

**Parameters:**
- `description` (optional): Description of what the screenshot shows
- `related_files` (optional): List of files related to this screenshot

### start_recording
Start video recording of the browser session.

**Parameters:**
- `description` (optional): Description of what the recording will show

### stop_recording
Stop the current video recording and save the MP4 file.

### list_assets
List all captured assets with metadata.

### delete_asset
Delete a specific asset by ID.

**Parameters:**
- `id` (required): The UUID of the asset to delete

### clear_assets
Delete all assets in the workspace.

## API Routes

### GET /api/task-attempts/{id}/assets
List all assets for a workspace.

**Response:**
```json
{
  "success": true,
  "data": {
    "assets": [...],
    "total": 3
  }
}
```

### GET /api/task-attempts/{id}/assets/{asset_id}
Get metadata for a specific asset.

### GET /api/task-attempts/{id}/assets/{asset_id}/file
Serve the asset file (image or video).

## PR Integration

When creating a PR, users can:
1. See available assets in the Create PR dialog
2. Select which assets to include
3. Selected assets are appended to the PR body as markdown

The PR body includes:
- For screenshots: Markdown image syntax `![Description](url)`
- For videos: A link to the video file (GitHub doesn't support video embedding)

**Note:** Assets are served via the vibe-kanban API. For assets to be viewable in the PR, the vibe-kanban instance must be accessible from wherever the PR is viewed.

## Frontend Components

### Hooks
- `useWorkspaceAssets(workspaceId)`: Fetches and polls for workspace assets

### Primitives
- `AssetThumbnail`: Displays a single asset as a clickable thumbnail
- `AssetPreview`: Lightbox/video player for viewing assets

### Containers
- `WorkspaceAssetsContainer`: Main assets gallery in the right sidebar

### Integration Points
- `RightSidebar.tsx`: Contains the collapsible Assets section
- `CreatePRDialog.tsx`: Asset selection for PR inclusion

## Key Files

### Backend
- `crates/utils/src/workspace_assets.rs`: Asset types and manifest handling
- `crates/services/src/services/workspace_assets.rs`: Asset service
- `crates/server/src/routes/task_attempts/assets.rs`: API routes
- `crates/server/src/routes/task_attempts/pr.rs`: PR creation with assets

### Frontend
- `frontend/src/hooks/useWorkspaceAssets.ts`: React hook for assets
- `frontend/src/components/ui-new/primitives/AssetThumbnail.tsx`
- `frontend/src/components/ui-new/primitives/AssetPreview.tsx`
- `frontend/src/components/ui-new/containers/WorkspaceAssetsContainer.tsx`
- `frontend/src/components/dialogs/tasks/CreatePRDialog.tsx`

### MCP Server
- `scripts/vibe-recorder/index.js`: Node.js MCP server
- `crates/executors/default_mcp.json`: MCP server configuration

## Design Decisions

- **Video Thumbnails**: Generic video icon placeholder (no ffmpeg dependency for thumbnails)
- **Size Limits**: No enforced limits - users manage storage themselves
- **Real-time Updates**: Asset gallery polls every 5 seconds while workspace is active
- **User Editing**: View-only - AI manages assets, users can only view
- **Git Integration**: `.vibe-assets/` includes `.gitignore` to prevent committing binaries

## Dependencies

- **ffmpeg**: Required for video encoding (must be installed on the system)
- **agent-browser**: Used for screenshots and video frame streaming
- **ws**: WebSocket client for connecting to agent-browser stream

## Setup

The `vibe_recorder` MCP server is included in the default MCP configuration. To enable asset capture:

1. Ensure `ffmpeg` is installed on the system
2. Ensure `agent-browser` is installed and accessible
3. The MCP server will be available to AI agents automatically

No additional configuration is required. Assets are stored per-workspace and automatically cleaned up when workspaces are deleted.
