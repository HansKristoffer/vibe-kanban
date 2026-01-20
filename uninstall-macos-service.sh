#!/bin/bash

# Uninstallation script for Vibe Kanban macOS LaunchAgent
# Also handles cleanup of old LaunchDaemon installations and auto-update service

set -e

SERVICE_NAME="com.vibekanban.server"
AUTO_UPDATE_SERVICE_NAME="com.vibekanban.autoupdate"
USER="${SUDO_USER:-$(whoami)}"
USER_HOME=$(eval echo "~$USER")
# LaunchAgent plist location
PLIST_PATH="${USER_HOME}/Library/LaunchAgents/${SERVICE_NAME}.plist"
AUTO_UPDATE_PLIST_PATH="${USER_HOME}/Library/LaunchAgents/${AUTO_UPDATE_SERVICE_NAME}.plist"
# Old LaunchDaemon location (for cleanup during migration)
OLD_DAEMON_PLIST="/Library/LaunchDaemons/${SERVICE_NAME}.plist"
INSTALL_DIR="/usr/local/bin/vibe-kanban"
MCP_INSTALL_DIR="/usr/local/bin/vibe-kanban-mcp"
AUTO_UPDATE_SCRIPT_INSTALL_DIR="/usr/local/bin/vibe-kanban-autoupdate"
SUDOERS_FILE="/etc/sudoers.d/vibe-kanban-autoupdate"
WORK_DIR="/var/vibe-kanban"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}Uninstalling Vibe Kanban service...${NC}"

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}Error: This script must be run as root (use sudo)${NC}"
    exit 1
fi

# Unload LaunchAgent if running (as target user)
if sudo -u "$USER" launchctl list 2>/dev/null | grep -q "$SERVICE_NAME"; then
    echo "Stopping LaunchAgent..."
    sudo -u "$USER" launchctl unload "$PLIST_PATH" 2>/dev/null || true
fi

# Unload auto-update LaunchAgent if running
if sudo -u "$USER" launchctl list 2>/dev/null | grep -q "$AUTO_UPDATE_SERVICE_NAME"; then
    echo "Stopping auto-update LaunchAgent..."
    sudo -u "$USER" launchctl unload "$AUTO_UPDATE_PLIST_PATH" 2>/dev/null || true
fi

# Also check for old LaunchDaemon
if launchctl list 2>/dev/null | grep -q "$SERVICE_NAME"; then
    echo "Stopping old LaunchDaemon..."
    launchctl unload "$OLD_DAEMON_PLIST" 2>/dev/null || true
fi

# Remove LaunchAgent plist
if [ -f "$PLIST_PATH" ]; then
    echo "Removing LaunchAgent plist..."
    rm -f "$PLIST_PATH"
fi

# Remove auto-update LaunchAgent plist
if [ -f "$AUTO_UPDATE_PLIST_PATH" ]; then
    echo "Removing auto-update LaunchAgent plist..."
    rm -f "$AUTO_UPDATE_PLIST_PATH"
fi

# Remove old LaunchDaemon plist if it exists
if [ -f "$OLD_DAEMON_PLIST" ]; then
    echo "Removing old LaunchDaemon plist..."
    rm -f "$OLD_DAEMON_PLIST"
fi

# Remove main binary
if [ -f "$INSTALL_DIR" ]; then
    echo "Removing binary..."
    rm -f "$INSTALL_DIR"
fi

# Remove MCP binary
if [ -f "$MCP_INSTALL_DIR" ]; then
    echo "Removing MCP binary..."
    rm -f "$MCP_INSTALL_DIR"
fi

# Remove auto-update script
if [ -f "$AUTO_UPDATE_SCRIPT_INSTALL_DIR" ]; then
    echo "Removing auto-update script..."
    rm -f "$AUTO_UPDATE_SCRIPT_INSTALL_DIR"
fi

# Remove sudoers entry for auto-update
if [ -f "$SUDOERS_FILE" ]; then
    echo "Removing sudoers entry..."
    rm -f "$SUDOERS_FILE"
fi

# Optionally remove working directory (ask first)
if [ -d "$WORK_DIR" ]; then
    read -p "Remove working directory ${WORK_DIR}? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$WORK_DIR"
        echo "Working directory removed."
    else
        echo "Working directory kept at ${WORK_DIR}"
    fi
fi

echo -e "${GREEN}Uninstallation complete!${NC}"
