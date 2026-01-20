#!/bin/bash

# Auto-update script for Vibe Kanban
# This script is called by the com.vibekanban.autoupdate LaunchAgent
# It checks for git updates and automatically reinstalls when changes are detected

set -e

# Configuration
WORK_DIR="/var/vibe-kanban"
REPO_PATH_FILE="${WORK_DIR}/.repo-path"
LOG_FILE="${WORK_DIR}/auto-update.log"
LOCK_FILE="${WORK_DIR}/.auto-update.lock"

# Logging helper
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" >> "$LOG_FILE"
}

# Ensure log directory exists
mkdir -p "$WORK_DIR"

# Check if repo path file exists
if [ ! -f "$REPO_PATH_FILE" ]; then
    log "ERROR: Repo path file not found at $REPO_PATH_FILE"
    exit 1
fi

REPO_PATH=$(cat "$REPO_PATH_FILE")

# Validate repo path
if [ ! -d "$REPO_PATH" ]; then
    log "ERROR: Repository path does not exist: $REPO_PATH"
    exit 1
fi

if [ ! -d "$REPO_PATH/.git" ]; then
    log "ERROR: Not a git repository: $REPO_PATH"
    exit 1
fi

# Check for lock file (prevent concurrent updates)
if [ -f "$LOCK_FILE" ]; then
    # Check if the lock is stale (older than 30 minutes)
    if [ "$(find "$LOCK_FILE" -mmin +30 2>/dev/null)" ]; then
        log "WARNING: Removing stale lock file"
        rm -f "$LOCK_FILE"
    else
        log "INFO: Another update is in progress, skipping"
        exit 0
    fi
fi

# Create lock file
echo $$ > "$LOCK_FILE"
trap 'rm -f "$LOCK_FILE"' EXIT

cd "$REPO_PATH"

# Get current branch
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
if [ -z "$CURRENT_BRANCH" ]; then
    log "ERROR: Could not determine current branch"
    exit 1
fi

log "INFO: Checking for updates on branch '$CURRENT_BRANCH'..."

# Fetch latest changes from remote
if ! git fetch origin "$CURRENT_BRANCH" 2>&1; then
    log "ERROR: Failed to fetch from origin"
    exit 1
fi

# Get local and remote commit hashes
LOCAL_COMMIT=$(git rev-parse HEAD)
REMOTE_COMMIT=$(git rev-parse "origin/$CURRENT_BRANCH" 2>/dev/null || git rev-parse "@{u}" 2>/dev/null)

if [ -z "$REMOTE_COMMIT" ]; then
    log "ERROR: Could not determine remote commit. Is the branch tracking a remote?"
    exit 1
fi

# Compare commits
if [ "$LOCAL_COMMIT" = "$REMOTE_COMMIT" ]; then
    log "INFO: Already up to date (commit: ${LOCAL_COMMIT:0:7})"
    exit 0
fi

log "INFO: Update available! Local: ${LOCAL_COMMIT:0:7} -> Remote: ${REMOTE_COMMIT:0:7}"

# Pull the latest changes
log "INFO: Pulling latest changes..."
if ! git pull origin "$CURRENT_BRANCH" 2>&1 | while read -r line; do log "GIT: $line"; done; then
    log "ERROR: Failed to pull changes"
    exit 1
fi

log "INFO: Pull successful. Starting reinstallation..."

# Run the install script with --skip-deps for faster updates
# --no-auto-update prevents recreating the auto-update service during update
INSTALL_SCRIPT="${REPO_PATH}/install-macos-service.sh"

if [ ! -f "$INSTALL_SCRIPT" ]; then
    log "ERROR: Install script not found at $INSTALL_SCRIPT"
    exit 1
fi

# Run install script using sudo (requires sudoers entry)
log "INFO: Running install script..."
if sudo "$INSTALL_SCRIPT" --skip-deps --no-auto-update 2>&1 | while read -r line; do log "INSTALL: $line"; done; then
    log "INFO: Update completed successfully!"
else
    log "ERROR: Install script failed with exit code $?"
    exit 1
fi
