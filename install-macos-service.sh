#!/bin/bash

# Install/Update script for Vibe Kanban as a macOS LaunchDaemon
# This script installs or updates vibe-kanban to run as a system service
# If the service already exists, it updates the binary while preserving configuration
#
# Usage:
#   sudo ./install-macos-service.sh [OPTIONS]
#
# Options:
#   --force     Force reinstall (recreates plist even if service exists)
#   --no-mcp    Skip installing the MCP server binary
#   --help      Show this help message

set -e

# Configuration
INSTALL_DIR="/usr/local/bin/vibe-kanban"
MCP_INSTALL_DIR="/usr/local/bin/vibe-kanban-mcp"
SERVICE_NAME="com.vibekanban.server"
PLIST_PATH="/Library/LaunchDaemons/${SERVICE_NAME}.plist"
USER="${SUDO_USER:-$(whoami)}"
USER_HOME=$(eval echo "~$USER")
PORT="${PORT:-3000}"
HOST="${HOST:-0.0.0.0}"
WORK_DIR="/var/vibe-kanban"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Parse arguments
FORCE_REINSTALL=false
INSTALL_MCP=true
for arg in "$@"; do
    case $arg in
        --force)
            FORCE_REINSTALL=true
            shift
            ;;
        --no-mcp)
            INSTALL_MCP=false
            shift
            ;;
        --help|-h)
            echo "Usage: sudo ./install-macos-service.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --force     Force reinstall (recreates plist even if service exists)"
            echo "  --no-mcp    Skip installing the MCP server binary"
            echo "  --help      Show this help message"
            echo ""
            echo "Environment variables:"
            echo "  PORT        Server port (default: 3000)"
            echo "  HOST        Server host (default: 0.0.0.0)"
            exit 0
            ;;
    esac
done

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}Error: This script must be run as root (use sudo)${NC}"
    exit 1
fi

# Pre-flight checks
echo -e "${CYAN}Running pre-flight checks...${NC}"

# Check for git (required by vibe-kanban)
if ! command -v git &> /dev/null; then
    echo -e "${RED}Error: git is not installed. Vibe Kanban requires git.${NC}"
    echo "Install with: brew install git"
    exit 1
fi

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
  x86_64)
    ARCH="x64"
    ;;
  arm64|aarch64)
    ARCH="arm64"
    ;;
  *)
    echo -e "${RED}Error: Unsupported architecture: $ARCH${NC}"
    exit 1
    ;;
esac

case "$OS" in
  darwin)
    OS="macos"
    ;;
  *)
    echo -e "${RED}Error: This script is for macOS only${NC}"
    exit 1
    ;;
esac

PLATFORM="${OS}-${ARCH}"
BINARY_DIR="npx-cli/dist/${PLATFORM}"

# Get version info
cd "$(dirname "$0")"
SCRIPT_DIR="$(pwd)"
VERSION=""
if [ -f "package.json" ]; then
    VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*"version": *"\([^"]*\)".*/\1/')
fi
GIT_COMMIT=""
if command -v git &> /dev/null && [ -d ".git" ]; then
    GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
fi

# Determine if this is an install or update
IS_UPDATE=false
if [ -f "$PLIST_PATH" ] && [ -f "$INSTALL_DIR" ]; then
    if [ "$FORCE_REINSTALL" = true ]; then
        echo -e "${YELLOW}Force reinstall requested. Will recreate service configuration.${NC}"
    else
        IS_UPDATE=true
    fi
fi

if [ "$IS_UPDATE" = true ]; then
    echo -e "${BLUE}Service found. Updating Vibe Kanban...${NC}"
else
    echo -e "${GREEN}Installing Vibe Kanban as a macOS service...${NC}"
fi

if [ -n "$VERSION" ]; then
    echo -e "Version: ${CYAN}${VERSION}${NC} (commit: ${GIT_COMMIT:-unknown})"
fi

# If updating, stop the service first
SERVICE_RUNNING=false
if [ "$IS_UPDATE" = true ] || [ "$FORCE_REINSTALL" = true ]; then
    if launchctl list 2>/dev/null | grep -q "$SERVICE_NAME"; then
        SERVICE_RUNNING=true
        echo "Stopping service..."
        launchctl unload "$PLIST_PATH" 2>/dev/null || true
        # Wait for process to stop
        for i in {1..10}; do
            if ! pgrep -f "$INSTALL_DIR" > /dev/null 2>&1; then
                break
            fi
            sleep 1
        done
    fi
fi

# Check if build exists, build if needed
if [ ! -d "$BINARY_DIR" ] || [ ! -f "${BINARY_DIR}/vibe-kanban.zip" ]; then
    echo -e "${YELLOW}Build not found. Building now...${NC}"
    ./local-build.sh
fi

# Extract binary
if [ ! -f "${BINARY_DIR}/vibe-kanban.zip" ]; then
    echo -e "${RED}Error: Binary not found at ${BINARY_DIR}/vibe-kanban.zip${NC}"
    echo "Please run ./local-build.sh first"
    exit 1
fi

echo "Extracting binary..."
cd "${BINARY_DIR}"
unzip -q -o vibe-kanban.zip
chmod +x vibe-kanban

# Extract MCP binary if requested
if [ "$INSTALL_MCP" = true ] && [ -f "vibe-kanban-mcp.zip" ]; then
    unzip -q -o vibe-kanban-mcp.zip
    chmod +x vibe-kanban-mcp
fi

# Create installation directory
mkdir -p "$(dirname "$INSTALL_DIR")"

# Backup old binary if updating
BACKUP_PATH=""
if [ "$IS_UPDATE" = true ] && [ -f "$INSTALL_DIR" ]; then
    BACKUP_PATH="${INSTALL_DIR}.backup.$(date +%Y%m%d_%H%M%S)"
    echo "Backing up old binary to ${BACKUP_PATH}..."
    cp "$INSTALL_DIR" "$BACKUP_PATH"
fi

# Install/Update main binary
echo "Installing binary to $INSTALL_DIR..."
cp vibe-kanban "$INSTALL_DIR"
chmod +x "$INSTALL_DIR"

# Install MCP binary if requested
if [ "$INSTALL_MCP" = true ] && [ -f "vibe-kanban-mcp" ]; then
    echo "Installing MCP binary to $MCP_INSTALL_DIR..."
    cp vibe-kanban-mcp "$MCP_INSTALL_DIR"
    chmod +x "$MCP_INSTALL_DIR"
fi

# Verify binary is executable
if [ ! -x "$INSTALL_DIR" ]; then
    echo -e "${RED}Error: Binary is not executable${NC}"
    if [ -n "$BACKUP_PATH" ] && [ -f "$BACKUP_PATH" ]; then
        echo "Restoring backup..."
        mv "$BACKUP_PATH" "$INSTALL_DIR"
    fi
    exit 1
fi

# Create working directory
mkdir -p "$WORK_DIR"
chown "$USER" "$WORK_DIR"

# Create or preserve LaunchDaemon plist
if [ "$IS_UPDATE" = false ]; then
    # Create new plist for fresh installation
    echo "Creating LaunchDaemon..."
    cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${SERVICE_NAME}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${INSTALL_DIR}</string>
    </array>
    <key>WorkingDirectory</key>
    <string>${WORK_DIR}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>${WORK_DIR}/vibe-kanban.log</string>
    <key>StandardErrorPath</key>
    <string>${WORK_DIR}/vibe-kanban.error.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOME</key>
        <string>${USER_HOME}</string>
        <key>HOST</key>
        <string>${HOST}</string>
        <key>PORT</key>
        <string>${PORT}</string>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>
    <key>UserName</key>
    <string>${USER}</string>
</dict>
</plist>
EOF

    # Set permissions
    chown root:wheel "$PLIST_PATH"
    chmod 644 "$PLIST_PATH"
else
    echo "Preserving existing service configuration..."
fi

# Load/Reload the service
echo "Starting service..."
launchctl load "$PLIST_PATH" 2>/dev/null || launchctl load -w "$PLIST_PATH"

# Wait for service to start and verify health
echo "Verifying service health..."
HEALTH_OK=false
for i in {1..15}; do
    sleep 1
    # Check if service is listed
    if launchctl list 2>/dev/null | grep -q "$SERVICE_NAME"; then
        # Try to reach the health endpoint
        if curl -s --connect-timeout 2 "http://127.0.0.1:${PORT}" > /dev/null 2>&1; then
            HEALTH_OK=true
            break
        fi
    fi
    printf "."
done
echo ""

if [ "$HEALTH_OK" = true ]; then
    echo -e "${GREEN}Service is healthy and responding on port ${PORT}${NC}"
else
    echo -e "${YELLOW}Warning: Could not verify service health. Check logs if issues persist.${NC}"
    echo "  tail -f ${WORK_DIR}/vibe-kanban.log"
fi

# Clean up old backups (keep last 3) if updating
if [ "$IS_UPDATE" = true ]; then
    echo "Cleaning up old backups..."
    find "$(dirname "$INSTALL_DIR")" -name "vibe-kanban.backup.*" -type f 2>/dev/null | sort -r | tail -n +4 | xargs rm -f 2>/dev/null || true
fi

# Success message
echo ""
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
if [ "$IS_UPDATE" = true ]; then
    echo -e "${GREEN}  Update complete!${NC}"
else
    echo -e "${GREEN}  Installation complete!${NC}"
fi
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Binary:    ${INSTALL_DIR}"
if [ "$INSTALL_MCP" = true ] && [ -f "$MCP_INSTALL_DIR" ]; then
    echo "  MCP:       ${MCP_INSTALL_DIR}"
fi
echo "  Service:   ${SERVICE_NAME}"
echo "  Logs:      ${WORK_DIR}/vibe-kanban.log"
echo "  Data:      ${USER_HOME}/Library/Application Support/ai.bloop.vibe-kanban/"
echo "  URL:       http://localhost:${PORT}"
echo ""
echo "Service management:"
echo "  Start:     sudo launchctl load ${PLIST_PATH}"
echo "  Stop:      sudo launchctl unload ${PLIST_PATH}"
echo "  Restart:   sudo launchctl unload ${PLIST_PATH} && sudo launchctl load ${PLIST_PATH}"
echo "  Status:    sudo launchctl list | grep ${SERVICE_NAME}"
echo "  Logs:      tail -f ${WORK_DIR}/vibe-kanban.log"
echo ""
if [ -n "$BACKUP_PATH" ]; then
    echo "Rollback:    sudo mv ${BACKUP_PATH} ${INSTALL_DIR}"
    echo ""
fi
echo "To update in the future, rebuild and run this script again:"
echo "  ./local-build.sh && sudo ./install-macos-service.sh"
echo ""
echo -e "${CYAN}Tip: Logs can grow large. Consider setting up log rotation with newsyslog or logrotate.${NC}"
