#!/usr/bin/env node

/**
 * Vibe Recorder - MCP Server for capturing screenshots and video recordings
 *
 * This MCP server provides tools for AI agents to document UI changes:
 * - screenshot: Capture a screenshot from agent-browser
 * - start-recording: Start video recording
 * - stop-recording: Stop video recording and save MP4
 * - list: List all captured assets
 * - delete: Delete a specific asset
 * - clear: Delete all assets
 */

import { spawn } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as readline from 'readline';
import { v4 as uuidv4 } from 'uuid';
import WebSocket from 'ws';

const VIBE_ASSETS_DIR = '.vibe-assets';
const MANIFEST_FILE = 'manifest.json';

// Recording state
let recordingState = {
  isRecording: false,
  ws: null,
  ffmpegProcess: null,
  outputPath: null,
  assetId: null,
  frameCount: 0,
  startTime: null,
};

/**
 * Get the workspace path from environment or current directory
 */
function getWorkspacePath() {
  return process.env.VIBE_WORKSPACE_PATH || process.cwd();
}

/**
 * Get the assets directory path
 */
function getAssetsDir() {
  return path.join(getWorkspacePath(), VIBE_ASSETS_DIR);
}

/**
 * Get the manifest file path
 */
function getManifestPath() {
  return path.join(getAssetsDir(), MANIFEST_FILE);
}

/**
 * Ensure the assets directory exists with .gitignore
 */
function ensureAssetsDir() {
  const assetsDir = getAssetsDir();
  if (!fs.existsSync(assetsDir)) {
    fs.mkdirSync(assetsDir, { recursive: true });
    // Create .gitignore to exclude assets from git
    fs.writeFileSync(path.join(assetsDir, '.gitignore'), '*\n');
  }
}

/**
 * Read the manifest file
 */
function readManifest() {
  const manifestPath = getManifestPath();
  if (!fs.existsSync(manifestPath)) {
    return { version: 1, assets: [] };
  }
  try {
    const content = fs.readFileSync(manifestPath, 'utf-8');
    return JSON.parse(content);
  } catch (e) {
    return { version: 1, assets: [] };
  }
}

/**
 * Write the manifest file
 */
function writeManifest(manifest) {
  ensureAssetsDir();
  const manifestPath = getManifestPath();
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
}

/**
 * Add an asset to the manifest
 */
function addAssetToManifest(entry) {
  const manifest = readManifest();
  manifest.assets.push(entry);
  writeManifest(manifest);
}

/**
 * Remove an asset from the manifest
 */
function removeAssetFromManifest(id) {
  const manifest = readManifest();
  const index = manifest.assets.findIndex((a) => a.id === id);
  if (index !== -1) {
    const removed = manifest.assets.splice(index, 1)[0];
    writeManifest(manifest);
    return removed;
  }
  return null;
}

/**
 * Take a screenshot using agent-browser
 */
async function takeScreenshot(description, relatedFiles = []) {
  ensureAssetsDir();

  const id = uuidv4();
  const filename = `${id}.png`;
  const outputPath = path.join(getAssetsDir(), filename);

  try {
    // Use agent-browser to take screenshot
    const result = await new Promise((resolve, reject) => {
      const proc = spawn('agent-browser', ['screenshot', outputPath], {
        stdio: ['pipe', 'pipe', 'pipe'],
      });

      let stdout = '';
      let stderr = '';

      proc.stdout.on('data', (data) => {
        stdout += data.toString();
      });
      proc.stderr.on('data', (data) => {
        stderr += data.toString();
      });

      proc.on('close', (code) => {
        if (code === 0) {
          resolve({ success: true, stdout, stderr });
        } else {
          reject(new Error(`agent-browser screenshot failed: ${stderr || stdout}`));
        }
      });

      proc.on('error', (err) => {
        reject(new Error(`Failed to spawn agent-browser: ${err.message}`));
      });
    });

    // Get file size
    const stats = fs.statSync(outputPath);

    // Add to manifest
    const entry = {
      id,
      asset_type: 'screenshot',
      filename,
      description: description || null,
      related_files: relatedFiles,
      captured_at: new Date().toISOString(),
      size_bytes: stats.size,
    };

    addAssetToManifest(entry);

    return {
      success: true,
      asset: entry,
      path: outputPath,
    };
  } catch (error) {
    // Clean up partial file if exists
    if (fs.existsSync(outputPath)) {
      fs.unlinkSync(outputPath);
    }
    throw error;
  }
}

/**
 * Start video recording
 */
async function startRecording(description) {
  if (recordingState.isRecording) {
    throw new Error('Recording already in progress');
  }

  ensureAssetsDir();

  const id = uuidv4();
  const filename = `${id}.mp4`;
  const outputPath = path.join(getAssetsDir(), filename);

  // Get the streaming port from environment or default
  const streamPort = process.env.AGENT_BROWSER_STREAM_PORT || '9223';
  const wsUrl = `ws://localhost:${streamPort}`;

  return new Promise((resolve, reject) => {
    try {
      // Connect to agent-browser WebSocket stream
      const ws = new WebSocket(wsUrl);

      ws.on('error', (err) => {
        reject(
          new Error(
            `Failed to connect to agent-browser stream at ${wsUrl}: ${err.message}. ` +
              `Make sure agent-browser is running with AGENT_BROWSER_STREAM_PORT=${streamPort}`
          )
        );
      });

      ws.on('open', () => {
        // Start ffmpeg to encode frames to MP4
        const ffmpegArgs = [
          '-y', // Overwrite output
          '-f',
          'image2pipe', // Input is pipe of images
          '-framerate',
          '10', // 10 fps
          '-i',
          '-', // Read from stdin
          '-c:v',
          'libx264', // H.264 codec
          '-pix_fmt',
          'yuv420p', // Pixel format for compatibility
          '-preset',
          'fast', // Encoding speed
          '-crf',
          '23', // Quality (lower = better, 23 is default)
          outputPath,
        ];

        const ffmpegProcess = spawn('ffmpeg', ffmpegArgs, {
          stdio: ['pipe', 'pipe', 'pipe'],
        });

        ffmpegProcess.on('error', (err) => {
          ws.close();
          reject(new Error(`Failed to start ffmpeg: ${err.message}. Make sure ffmpeg is installed.`));
        });

        ffmpegProcess.stderr.on('data', (data) => {
          // ffmpeg outputs progress to stderr, we can log it if needed
        });

        recordingState = {
          isRecording: true,
          ws,
          ffmpegProcess,
          outputPath,
          assetId: id,
          filename,
          description,
          frameCount: 0,
          startTime: Date.now(),
        };

        resolve({
          success: true,
          message: 'Recording started',
          assetId: id,
        });
      });

      ws.on('message', (data) => {
        try {
          const message = JSON.parse(data.toString());
          if (message.type === 'frame' && message.data) {
            // Decode base64 JPEG frame and pipe to ffmpeg
            const frameBuffer = Buffer.from(message.data, 'base64');
            if (
              recordingState.ffmpegProcess &&
              recordingState.ffmpegProcess.stdin.writable
            ) {
              recordingState.ffmpegProcess.stdin.write(frameBuffer);
              recordingState.frameCount++;
            }
          }
        } catch (e) {
          // Ignore malformed messages
        }
      });

      ws.on('close', () => {
        if (recordingState.isRecording) {
          // Unexpected close, stop recording
          stopRecordingInternal();
        }
      });
    } catch (error) {
      reject(error);
    }
  });
}

/**
 * Internal function to stop recording and finalize the video
 */
async function stopRecordingInternal() {
  if (!recordingState.isRecording) {
    return null;
  }

  const {
    ws,
    ffmpegProcess,
    outputPath,
    assetId,
    filename,
    description,
    frameCount,
    startTime,
  } = recordingState;

  // Close WebSocket
  if (ws) {
    ws.close();
  }

  // Close ffmpeg stdin and wait for it to finish
  if (ffmpegProcess) {
    return new Promise((resolve, reject) => {
      ffmpegProcess.stdin.end();

      ffmpegProcess.on('close', (code) => {
        const durationMs = Date.now() - startTime;

        // Reset state
        recordingState = {
          isRecording: false,
          ws: null,
          ffmpegProcess: null,
          outputPath: null,
          assetId: null,
          frameCount: 0,
          startTime: null,
        };

        if (code === 0 && fs.existsSync(outputPath)) {
          const stats = fs.statSync(outputPath);

          // Add to manifest
          const entry = {
            id: assetId,
            asset_type: 'video',
            filename,
            description: description || null,
            related_files: [],
            captured_at: new Date().toISOString(),
            duration_ms: durationMs,
            size_bytes: stats.size,
          };

          addAssetToManifest(entry);

          resolve({
            success: true,
            asset: entry,
            path: outputPath,
            frameCount,
          });
        } else {
          // Clean up partial file
          if (fs.existsSync(outputPath)) {
            fs.unlinkSync(outputPath);
          }
          reject(new Error(`ffmpeg exited with code ${code}`));
        }
      });
    });
  }

  // Reset state even if no ffmpeg process
  recordingState = {
    isRecording: false,
    ws: null,
    ffmpegProcess: null,
    outputPath: null,
    assetId: null,
    frameCount: 0,
    startTime: null,
  };

  return null;
}

/**
 * Stop video recording
 */
async function stopRecording() {
  if (!recordingState.isRecording) {
    throw new Error('No recording in progress');
  }

  return await stopRecordingInternal();
}

/**
 * List all assets
 */
function listAssets() {
  const manifest = readManifest();
  return manifest.assets;
}

/**
 * Delete an asset by ID
 */
function deleteAsset(id) {
  const removed = removeAssetFromManifest(id);
  if (!removed) {
    throw new Error(`Asset not found: ${id}`);
  }

  // Delete the file
  const filePath = path.join(getAssetsDir(), removed.filename);
  if (fs.existsSync(filePath)) {
    fs.unlinkSync(filePath);
  }

  return removed;
}

/**
 * Clear all assets
 */
function clearAssets() {
  const manifest = readManifest();
  const count = manifest.assets.length;

  // Delete all files
  for (const asset of manifest.assets) {
    const filePath = path.join(getAssetsDir(), asset.filename);
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
    }
  }

  // Clear manifest
  manifest.assets = [];
  writeManifest(manifest);

  return { deleted: count };
}

// MCP Protocol Implementation

const TOOLS = [
  {
    name: 'screenshot',
    description:
      'Capture a screenshot of the current browser state. Use this to document UI changes, capture error states, or show the result of your work.',
    inputSchema: {
      type: 'object',
      properties: {
        description: {
          type: 'string',
          description: 'Description of what this screenshot shows (e.g., "Login page after styling changes")',
        },
        related_files: {
          type: 'array',
          items: { type: 'string' },
          description: 'List of files related to this screenshot (files you modified)',
        },
      },
    },
  },
  {
    name: 'start_recording',
    description:
      'Start recording a video of the browser session. Use this before making UI changes to capture the entire workflow. The video will capture frames from agent-browser.',
    inputSchema: {
      type: 'object',
      properties: {
        description: {
          type: 'string',
          description: 'Description of what this recording will show',
        },
      },
    },
  },
  {
    name: 'stop_recording',
    description:
      'Stop the current video recording and save the MP4 file. Call this after completing the UI changes you wanted to document.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'list_assets',
    description:
      'List all captured assets (screenshots and videos) in the workspace. Returns metadata including ID, type, description, and file size.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'delete_asset',
    description: 'Delete a specific asset by its ID.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'string',
          description: 'The UUID of the asset to delete',
        },
      },
      required: ['id'],
    },
  },
  {
    name: 'clear_assets',
    description:
      'Delete all assets in the workspace. Use this to start fresh before documenting new changes.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
];

/**
 * Handle MCP tool calls
 */
async function handleToolCall(name, args) {
  switch (name) {
    case 'screenshot':
      return await takeScreenshot(args.description, args.related_files);

    case 'start_recording':
      return await startRecording(args.description);

    case 'stop_recording':
      return await stopRecording();

    case 'list_assets':
      return { assets: listAssets() };

    case 'delete_asset':
      return { deleted: deleteAsset(args.id) };

    case 'clear_assets':
      return clearAssets();

    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

/**
 * Send a JSON-RPC response
 */
function sendResponse(id, result) {
  const response = {
    jsonrpc: '2.0',
    id,
    result,
  };
  console.log(JSON.stringify(response));
}

/**
 * Send a JSON-RPC error
 */
function sendError(id, code, message) {
  const response = {
    jsonrpc: '2.0',
    id,
    error: { code, message },
  };
  console.log(JSON.stringify(response));
}

/**
 * Handle incoming JSON-RPC messages
 */
async function handleMessage(message) {
  try {
    const request = JSON.parse(message);
    const { id, method, params } = request;

    switch (method) {
      case 'initialize':
        sendResponse(id, {
          protocolVersion: '2024-11-05',
          capabilities: {
            tools: {},
          },
          serverInfo: {
            name: 'vibe-recorder',
            version: '1.0.0',
          },
        });
        break;

      case 'tools/list':
        sendResponse(id, { tools: TOOLS });
        break;

      case 'tools/call':
        try {
          const result = await handleToolCall(params.name, params.arguments || {});
          sendResponse(id, {
            content: [
              {
                type: 'text',
                text: JSON.stringify(result, null, 2),
              },
            ],
          });
        } catch (error) {
          sendResponse(id, {
            content: [
              {
                type: 'text',
                text: `Error: ${error.message}`,
              },
            ],
            isError: true,
          });
        }
        break;

      case 'notifications/initialized':
        // Client acknowledged initialization, no response needed
        break;

      default:
        if (id !== undefined) {
          sendError(id, -32601, `Method not found: ${method}`);
        }
    }
  } catch (error) {
    // Parse error
    sendError(null, -32700, `Parse error: ${error.message}`);
  }
}

/**
 * Main entry point
 */
async function main() {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false,
  });

  rl.on('line', async (line) => {
    if (line.trim()) {
      await handleMessage(line);
    }
  });

  rl.on('close', () => {
    // Clean up any active recording
    if (recordingState.isRecording) {
      stopRecordingInternal();
    }
    process.exit(0);
  });
}

main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
