#!/bin/bash

# Uninstallation script for Vibe Kanban macOS LaunchDaemon

set -e

SERVICE_NAME="com.vibekanban.server"
PLIST_PATH="/Library/LaunchDaemons/${SERVICE_NAME}.plist"
INSTALL_DIR="/usr/local/bin/vibe-kanban"
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

# Unload service if running
if launchctl list | grep -q "$SERVICE_NAME"; then
    echo "Stopping service..."
    launchctl unload "$PLIST_PATH" 2>/dev/null || true
fi

# Remove plist
if [ -f "$PLIST_PATH" ]; then
    echo "Removing LaunchDaemon plist..."
    rm -f "$PLIST_PATH"
fi

# Remove binary
if [ -f "$INSTALL_DIR" ]; then
    echo "Removing binary..."
    rm -f "$INSTALL_DIR"
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
