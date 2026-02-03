#!/bin/bash

# Install/Update script for Vibe Kanban as a macOS LaunchAgent
# This script installs or updates vibe-kanban to run as a user service
# LaunchAgents run in the user session with full keychain access (required for Claude Code OAuth)
# If the service already exists, it updates the binary while preserving configuration
#
# This script automatically handles all initial setup:
#   - Installs Homebrew (if not present)
#   - Installs Node.js via Homebrew
#   - Installs pnpm globally via npm
#   - Installs Rust via rustup
#   - Runs pnpm install
#   - Generates TypeScript types
#   - Builds the project (via local-build.sh)
#
# Usage:
#   sudo ./install-macos-service.sh [OPTIONS]
#
# Options:
#   --force     Force reinstall (recreates plist even if service exists)
#   --no-mcp    Skip installing the MCP server binary
#   --skip-deps Skip dependency installation (Homebrew, Node, pnpm, Rust)
#   --tailscale-funnel  Enable Tailscale Funnel setup (public URL)
#   --no-auto-update    Disable auto-update daemon (polls git for changes)
#   --help      Show this help message
#
# Note: sudo is required for installing binaries to /usr/local/bin and creating /var/vibe-kanban
# The LaunchAgent itself runs without elevated privileges in the user session.

set -e

# Helpers
is_truthy() {
    case "${1:-}" in
        1|true|TRUE|yes|YES|y|Y|on|ON) return 0 ;;
        *) return 1 ;;
    esac
}

# Configuration
INSTALL_DIR="/usr/local/bin/vibe-kanban"
MCP_INSTALL_DIR="/usr/local/bin/vibe-kanban-mcp"
AUTO_UPDATE_SCRIPT_INSTALL_DIR="/usr/local/bin/vibe-kanban-autoupdate"
SERVICE_NAME="com.vibekanban.server"
AUTO_UPDATE_SERVICE_NAME="com.vibekanban.autoupdate"
USER="${SUDO_USER:-$(whoami)}"
USER_HOME=$(eval echo "~$USER")
# LaunchAgent plist goes in user's LaunchAgents directory
PLIST_PATH="${USER_HOME}/Library/LaunchAgents/${SERVICE_NAME}.plist"
AUTO_UPDATE_PLIST_PATH="${USER_HOME}/Library/LaunchAgents/${AUTO_UPDATE_SERVICE_NAME}.plist"
# Also track old daemon path for migration
OLD_DAEMON_PLIST="/Library/LaunchDaemons/${SERVICE_NAME}.plist"
WORK_DIR="/var/vibe-kanban"
REPO_PATH_FILE="${WORK_DIR}/.repo-path"
# Auto-update interval in seconds (15 minutes)
AUTO_UPDATE_INTERVAL=900

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
SKIP_DEPS=false
ENABLE_TAILSCALE_FUNNEL=false
ENABLE_AUTO_UPDATE=true
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
        --skip-deps)
            SKIP_DEPS=true
            shift
            ;;
        --tailscale-funnel)
            ENABLE_TAILSCALE_FUNNEL=true
            shift
            ;;
        --no-auto-update)
            ENABLE_AUTO_UPDATE=false
            shift
            ;;
        --help|-h)
            echo "Usage: sudo ./install-macos-service.sh [OPTIONS]"
            echo ""
            echo "Installs Vibe Kanban as a LaunchAgent (user service with keychain access)."
            echo "This allows Claude Code OAuth authentication to work properly."
            echo ""
            echo "This script automatically handles all initial setup:"
            echo "  - Installs Homebrew (if not present)"
            echo "  - Installs Node.js via Homebrew"
            echo "  - Installs pnpm globally via npm"
            echo "  - Installs Rust via rustup"
            echo "  - Runs pnpm install"
            echo "  - Generates TypeScript types"
            echo "  - Builds the project"
            echo ""
            echo "Options:"
            echo "  --force      Force reinstall (recreates plist even if service exists)"
            echo "  --no-mcp     Skip installing the MCP server binary"
            echo "  --skip-deps  Skip dependency installation (Homebrew, Node, pnpm, Rust)"
            echo "  --tailscale-funnel  Enable Tailscale Funnel setup (or use TAILSCALE_FUNNEL=1 in .env)"
            echo "  --no-auto-update    Disable auto-update daemon (or use AUTO_UPDATE=0 in .env)"
            echo "  --help       Show this help message"
            echo ""
            echo "Configuration is done via .env file. Copy .env.example to .env and edit:"
            echo "  cp .env.example .env"
            echo ""
            echo "Required variables (in .env):"
            echo "  VK_PUBLIC_BASE_URL      Public URL where the service is accessible"
            echo "  GOOGLE_CLIENT_ID        Google OAuth client ID"
            echo "  GOOGLE_CLIENT_SECRET    Google OAuth client secret"
            echo ""
            echo "Optional variables (in .env):"
            echo "  PORT              Server port (default: 3000)"
            echo "  HOST              Server host (default: 0.0.0.0; auto: 127.0.0.1 with Funnel)"
            echo "  TAILSCALE_FUNNEL  Set to 1 to enable Tailscale Funnel for public HTTPS"
            echo "  AUTO_UPDATE       Set to 0 to disable auto-update daemon (default: 1)"
            echo "  AUTH_DISABLED     Set to 1 to disable authentication (no Google login needed)"
            echo ""
            echo "Example:"
            echo "  sudo ./install-macos-service.sh"
            echo "  sudo ./install-macos-service.sh --force"
            echo "  sudo ./install-macos-service.sh --skip-deps  # If you already have all dependencies"
            exit 0
            ;;
    esac
done

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}Error: This script must be run as root (use sudo)${NC}"
    exit 1
fi

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load .env file FIRST (before checking environment variables)
# This ensures .env values are available for HOST/PORT/TAILSCALE_FUNNEL defaults
ENV_FILE="${SCRIPT_DIR:-.}/.env"
if [ -f "$ENV_FILE" ]; then
    echo -e "${CYAN}Loading environment from .env file...${NC}"
    # Read .env file and export variables (handles both KEY=value and export KEY=value formats)
    while IFS= read -r line || [ -n "$line" ]; do
        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
        # Remove 'export ' prefix if present
        line="${line#export }"
        # Only process lines with = sign
        if [[ "$line" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; then
            # Extract variable name
            var_name="${line%%=*}"
            # Only set if not already set in environment (allows overrides)
            if [ -z "${!var_name:-}" ]; then
                export "$line"
            fi
        fi
    done < "$ENV_FILE"
fi

# Determine whether HOST was explicitly set by caller (after .env loading)
HOST_WAS_SET=false
if [ -n "${HOST+x}" ]; then
    HOST_WAS_SET=true
fi

# Read environment variables (now .env values are available)
PORT="${PORT:-3000}"
if is_truthy "${TAILSCALE_FUNNEL:-}"; then
    ENABLE_TAILSCALE_FUNNEL=true
fi
# Check AUTO_UPDATE from .env (default is enabled, so we check for explicit disable)
if [ -n "${AUTO_UPDATE:-}" ]; then
    if ! is_truthy "${AUTO_UPDATE}"; then
        ENABLE_AUTO_UPDATE=false
    fi
fi

# When enabling Funnel, default HOST to loopback unless explicitly provided.
DEFAULT_HOST="0.0.0.0"
if [ "$ENABLE_TAILSCALE_FUNNEL" = true ] && [ "$HOST_WAS_SET" = false ]; then
    DEFAULT_HOST="127.0.0.1"
fi
HOST="${HOST:-$DEFAULT_HOST}"

# Required environment variables (Google OAuth not required when AUTH_DISABLED)
if is_truthy "${AUTH_DISABLED:-}"; then
    REQUIRED_VARS=(
        "VK_PUBLIC_BASE_URL"
    )
else
    REQUIRED_VARS=(
        "VK_PUBLIC_BASE_URL"
        "GOOGLE_CLIENT_ID"
        "GOOGLE_CLIENT_SECRET"
    )
fi

# Check required environment variables
MISSING_VARS=()
for var in "${REQUIRED_VARS[@]}"; do
    if [ -z "${!var:-}" ]; then
        MISSING_VARS+=("$var")
    fi
done

if [ ${#MISSING_VARS[@]} -gt 0 ]; then
    echo -e "${RED}Error: Missing required environment variables:${NC}"
    for var in "${MISSING_VARS[@]}"; do
        echo -e "  - ${YELLOW}${var}${NC}"
    done
    echo ""
    echo "Create a .env file in the project directory with these variables,"
    echo "or set them before running this script:"
    echo ""
    echo -e "  ${CYAN}VK_PUBLIC_BASE_URL=https://example.com \\\\${NC}"
    echo -e "  ${CYAN}GOOGLE_CLIENT_ID=... \\\\${NC}"
    echo -e "  ${CYAN}GOOGLE_CLIENT_SECRET=... \\\\${NC}"
    echo -e "  ${CYAN}sudo ./install-macos-service.sh${NC}"
    exit 1
fi

# Pre-flight checks
echo -e "${CYAN}Running pre-flight checks...${NC}"

# Helper function to run commands as the non-root user
run_as_user() {
    sudo -u "$USER" "$@"
}

# Helper function to check if a command exists for the user
user_has_command() {
    sudo -u "$USER" bash -c "command -v $1" &> /dev/null
}

# Helper function to get user's shell profile
get_shell_profile() {
    local shell_name=$(basename "$SHELL")
    case "$shell_name" in
        zsh)  echo "${USER_HOME}/.zshrc" ;;
        bash) echo "${USER_HOME}/.bash_profile" ;;
        *)    echo "${USER_HOME}/.profile" ;;
    esac
}

# Ensure brew is in PATH for sudo commands
BREW_PATH=""
if [ -f "/opt/homebrew/bin/brew" ]; then
    BREW_PATH="/opt/homebrew/bin"
elif [ -f "/usr/local/bin/brew" ]; then
    BREW_PATH="/usr/local/bin"
fi

# Ensure cargo is in PATH for subsequent commands
export PATH="${USER_HOME}/.cargo/bin:${BREW_PATH}:$PATH"

if [ "$SKIP_DEPS" = false ]; then
    # ============================================================
    # DEPENDENCY INSTALLATION SECTION
    # ============================================================
    echo -e "${CYAN}Checking and installing dependencies...${NC}"

    # Check for Homebrew
    if ! user_has_command brew; then
        echo -e "${YELLOW}Homebrew not found. Installing Homebrew...${NC}"
        # Install Homebrew as the non-root user
        run_as_user /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        
        # Add Homebrew to PATH for this session
        if [ -f "/opt/homebrew/bin/brew" ]; then
            BREW_PATH="/opt/homebrew/bin"
            eval "$(/opt/homebrew/bin/brew shellenv)"
        elif [ -f "/usr/local/bin/brew" ]; then
            BREW_PATH="/usr/local/bin"
            eval "$(/usr/local/bin/brew shellenv)"
        fi
        
        echo -e "${GREEN}Homebrew installed successfully.${NC}"
    else
        echo -e "${GREEN}✓ Homebrew is installed${NC}"
    fi

    # Check for git (required by vibe-kanban)
    if ! command -v git &> /dev/null; then
        echo -e "${YELLOW}git not found. Installing git via Homebrew...${NC}"
        run_as_user "${BREW_PATH}/brew" install git
        echo -e "${GREEN}git installed successfully.${NC}"
    else
        echo -e "${GREEN}✓ git is installed${NC}"
    fi

    # Check for Node.js
    if ! user_has_command node; then
        echo -e "${YELLOW}Node.js not found. Installing Node.js via Homebrew...${NC}"
        run_as_user "${BREW_PATH}/brew" install node
        echo -e "${GREEN}Node.js installed successfully.${NC}"
    else
        echo -e "${GREEN}✓ Node.js is installed ($(node --version))${NC}"
    fi

    # Check for pnpm
    if ! user_has_command pnpm; then
        echo -e "${YELLOW}pnpm not found. Installing pnpm globally via npm...${NC}"
        run_as_user npm install -g pnpm
        echo -e "${GREEN}pnpm installed successfully.${NC}"
    else
        echo -e "${GREEN}✓ pnpm is installed${NC}"
    fi

    # Check for Rust
    if ! user_has_command rustc; then
        echo -e "${YELLOW}Rust not found. Installing Rust via rustup...${NC}"
        # Install rustup non-interactively
        run_as_user bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
        
        # Source cargo env for this session
        if [ -f "${USER_HOME}/.cargo/env" ]; then
            source "${USER_HOME}/.cargo/env"
        fi
        # Update PATH
        export PATH="${USER_HOME}/.cargo/bin:$PATH"
        
        echo -e "${GREEN}Rust installed successfully.${NC}"
    else
        echo -e "${GREEN}✓ Rust is installed ($(rustc --version | cut -d' ' -f2))${NC}"
    fi

    # ============================================================
    # PROJECT SETUP SECTION
    # ============================================================
    echo ""
    echo -e "${CYAN}Setting up project dependencies...${NC}"

    # Always run pnpm install to ensure dependencies are up to date
    # (pnpm is fast when nothing changed)
    echo -e "${YELLOW}Installing/updating npm dependencies with pnpm...${NC}"
    cd "$SCRIPT_DIR"
    run_as_user pnpm install
    echo -e "${GREEN}Dependencies installed successfully.${NC}"

    # Always regenerate TypeScript types to ensure they match current Rust code
    echo -e "${YELLOW}Generating TypeScript types...${NC}"
    cd "$SCRIPT_DIR"
    run_as_user pnpm run generate-types
    echo -e "${GREEN}Types generated successfully.${NC}"

    # Always rebuild to ensure binary matches current code
    echo ""
    echo -e "${YELLOW}Building project...${NC}"
    cd "$SCRIPT_DIR"
    ./local-build.sh
    echo -e "${GREEN}Build completed successfully.${NC}"

    echo ""
    echo -e "${GREEN}All dependencies are ready!${NC}"
    echo ""
else
    echo -e "${YELLOW}Skipping dependency installation (--skip-deps)${NC}"
    
    # Still check for critical dependencies
    if ! command -v git &> /dev/null; then
        echo -e "${RED}Error: git is not installed. Vibe Kanban requires git.${NC}"
        echo "Install with: brew install git"
        exit 1
    fi
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
cd "$SCRIPT_DIR"
VERSION=""
if [ -f "package.json" ]; then
    VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*"version": *"\([^"]*\)".*/\1/')
fi
GIT_COMMIT=""
if command -v git &> /dev/null && [ -d ".git" ]; then
    GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
fi

# Migrate from old LaunchDaemon if it exists
if [ -f "$OLD_DAEMON_PLIST" ]; then
    echo -e "${YELLOW}Found old LaunchDaemon installation. Migrating to LaunchAgent...${NC}"
    # Stop and remove old daemon
    if launchctl list 2>/dev/null | grep -q "$SERVICE_NAME"; then
        echo "Stopping old LaunchDaemon..."
        launchctl unload "$OLD_DAEMON_PLIST" 2>/dev/null || true
        # Wait for process to stop
        for i in {1..10}; do
            if ! pgrep -f "$INSTALL_DIR" > /dev/null 2>&1; then
                break
            fi
            sleep 1
        done
    fi
    echo "Removing old LaunchDaemon plist..."
    rm -f "$OLD_DAEMON_PLIST"
    # Force reinstall to create new LaunchAgent
    FORCE_REINSTALL=true
fi

# Create LaunchAgents directory if it doesn't exist
mkdir -p "${USER_HOME}/Library/LaunchAgents"
chown "$USER" "${USER_HOME}/Library/LaunchAgents"

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
    echo -e "${GREEN}Installing Vibe Kanban as a macOS LaunchAgent...${NC}"
fi

if [ -n "$VERSION" ]; then
    echo -e "Version: ${CYAN}${VERSION}${NC} (commit: ${GIT_COMMIT:-unknown})"
fi

# If updating, stop the service first
SERVICE_RUNNING=false
if [ "$IS_UPDATE" = true ] || [ "$FORCE_REINSTALL" = true ]; then
    # Check if service is running (as the target user, not root)
    if sudo -u "$USER" launchctl list 2>/dev/null | grep -q "$SERVICE_NAME"; then
        SERVICE_RUNNING=true
        echo "Stopping service..."
        sudo -u "$USER" launchctl unload "$PLIST_PATH" 2>/dev/null || true
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

# Store repository path for auto-update script
echo "$SCRIPT_DIR" > "$REPO_PATH_FILE"
chown "$USER" "$REPO_PATH_FILE"

# Create or preserve LaunchAgent plist
if [ "$IS_UPDATE" = false ]; then
    # Create new plist for fresh installation
    echo "Creating LaunchAgent..."
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
    <key>SoftResourceLimits</key>
    <dict>
        <key>NumberOfFiles</key>
        <integer>65536</integer>
    </dict>
    <key>HardResourceLimits</key>
    <dict>
        <key>NumberOfFiles</key>
        <integer>65536</integer>
    </dict>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOST</key>
        <string>${HOST}</string>
        <key>PORT</key>
        <string>${PORT}</string>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
        <key>VK_PUBLIC_BASE_URL</key>
        <string>${VK_PUBLIC_BASE_URL}</string>
        <key>GOOGLE_CLIENT_ID</key>
        <string>${GOOGLE_CLIENT_ID:-}</string>
        <key>GOOGLE_CLIENT_SECRET</key>
        <string>${GOOGLE_CLIENT_SECRET:-}</string>
        <key>AUTH_DISABLED</key>
        <string>${AUTH_DISABLED:-}</string>
    </dict>
</dict>
</plist>
EOF

    # Set permissions (owned by user, not root)
    chown "$USER" "$PLIST_PATH"
    chmod 644 "$PLIST_PATH"
else
    echo "Preserving existing service configuration..."
fi

# Load/Reload the service (as the target user, not root)
echo "Starting service..."
sudo -u "$USER" launchctl load "$PLIST_PATH" 2>/dev/null || sudo -u "$USER" launchctl load -w "$PLIST_PATH"

# Wait for service to start and verify health
echo "Verifying service health..."
HEALTH_OK=false
for i in {1..15}; do
    sleep 1
    # Check if service is listed (as target user)
    if sudo -u "$USER" launchctl list 2>/dev/null | grep -q "$SERVICE_NAME"; then
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

    if [ "$ENABLE_TAILSCALE_FUNNEL" = true ]; then
        echo ""
        echo -e "${CYAN}Configuring Tailscale Funnel...${NC}"
        echo -e "${YELLOW}Warning: Funnel creates a public internet URL.${NC}"

        if ! command -v tailscale &> /dev/null; then
            echo -e "${YELLOW}Warning: tailscale CLI not found. Skipping Funnel setup.${NC}"
            echo "Install Tailscale and run: sudo tailscale funnel --bg --yes ${PORT}"
        elif ! tailscale status >/dev/null 2>&1; then
            echo -e "${YELLOW}Warning: Tailscale is not running/logged in. Skipping Funnel setup.${NC}"
            echo "Run: sudo tailscale status"
            echo "Then: sudo tailscale up"
        elif ! tailscale funnel --help >/dev/null 2>&1; then
            echo -e "${YELLOW}Warning: Your Tailscale version does not support Funnel. Skipping Funnel setup.${NC}"
            echo "Upgrade Tailscale and then run: sudo tailscale funnel --bg --yes ${PORT}"
        elif ! tailscale funnel --help 2>&1 | grep -q -- '--bg'; then
            echo -e "${YELLOW}Warning: Your Tailscale CLI does not support --bg for Funnel. Skipping Funnel setup.${NC}"
            echo "Run manually from a terminal: sudo tailscale funnel ${PORT}"
        elif ! tailscale funnel --help 2>&1 | grep -q -- '--yes'; then
            echo -e "${YELLOW}Warning: Your Tailscale CLI does not support non-interactive Funnel setup (--yes). Skipping Funnel setup.${NC}"
            echo "Run manually from a terminal: sudo tailscale funnel ${PORT}"
        else
            if tailscale funnel --bg --yes "${PORT}"; then
                echo -e "${GREEN}Tailscale Funnel enabled.${NC}"
                FUNNEL_STATUS="$(tailscale funnel status 2>/dev/null || true)"
                if [ -n "$FUNNEL_STATUS" ]; then
                    echo "$FUNNEL_STATUS"
                    FUNNEL_URL="$(echo "$FUNNEL_STATUS" | grep -Eo 'https://[^[:space:]]+' | head -1 || true)"
                    if [ -n "$FUNNEL_URL" ]; then
                        echo -e "${GREEN}Public URL: ${FUNNEL_URL}${NC}"
                    fi
                else
                    tailscale funnel status 2>/dev/null || true
                fi
            else
                echo -e "${YELLOW}Warning: Failed to enable Tailscale Funnel.${NC}"
                echo "You can retry with: sudo tailscale funnel --bg --yes ${PORT}"
                echo "Check: sudo tailscale funnel status"
            fi
        fi
    fi
else
    echo -e "${YELLOW}Warning: Could not verify service health. Check logs if issues persist.${NC}"
    echo "  tail -f ${WORK_DIR}/vibe-kanban.log"
fi

# ============================================================
# AUTO-UPDATE SERVICE SETUP
# ============================================================
if [ "$ENABLE_AUTO_UPDATE" = true ]; then
    echo ""
    echo -e "${CYAN}Setting up auto-update service...${NC}"
    
    # Install auto-update script
    AUTO_UPDATE_SCRIPT="${SCRIPT_DIR}/auto-update.sh"
    if [ -f "$AUTO_UPDATE_SCRIPT" ]; then
        echo "Installing auto-update script to $AUTO_UPDATE_SCRIPT_INSTALL_DIR..."
        cp "$AUTO_UPDATE_SCRIPT" "$AUTO_UPDATE_SCRIPT_INSTALL_DIR"
        chmod +x "$AUTO_UPDATE_SCRIPT_INSTALL_DIR"
        chown "$USER" "$AUTO_UPDATE_SCRIPT_INSTALL_DIR"
    else
        echo -e "${YELLOW}Warning: Auto-update script not found at $AUTO_UPDATE_SCRIPT${NC}"
        ENABLE_AUTO_UPDATE=false
    fi
fi

if [ "$ENABLE_AUTO_UPDATE" = true ]; then
    # Stop existing auto-update service if running
    if sudo -u "$USER" launchctl list 2>/dev/null | grep -q "$AUTO_UPDATE_SERVICE_NAME"; then
        echo "Stopping existing auto-update service..."
        sudo -u "$USER" launchctl unload "$AUTO_UPDATE_PLIST_PATH" 2>/dev/null || true
    fi
    
    # Create sudoers entry for passwordless install script execution
    SUDOERS_FILE="/etc/sudoers.d/vibe-kanban-autoupdate"
    echo "Setting up passwordless sudo for auto-update..."
    
    # Create sudoers entry that allows user to run install script without password
    cat > "$SUDOERS_FILE" <<EOF
# Allow ${USER} to run the Vibe Kanban install script without a password
# This is required for the auto-update daemon to work
${USER} ALL=(ALL) NOPASSWD: ${SCRIPT_DIR}/install-macos-service.sh
EOF
    
    # Set proper permissions (must be 0440 for sudoers.d files)
    chmod 0440 "$SUDOERS_FILE"
    
    # Validate sudoers file
    if ! visudo -c -f "$SUDOERS_FILE" > /dev/null 2>&1; then
        echo -e "${YELLOW}Warning: Sudoers file validation failed. Removing...${NC}"
        rm -f "$SUDOERS_FILE"
        echo -e "${YELLOW}Auto-update may require manual password entry.${NC}"
    else
        echo -e "${GREEN}Sudoers entry created successfully.${NC}"
    fi
    
    # Create auto-update LaunchAgent plist
    echo "Creating auto-update LaunchAgent..."
    cat > "$AUTO_UPDATE_PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${AUTO_UPDATE_SERVICE_NAME}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${AUTO_UPDATE_SCRIPT_INSTALL_DIR}</string>
    </array>
    <key>WorkingDirectory</key>
    <string>${WORK_DIR}</string>
    <key>StartInterval</key>
    <integer>${AUTO_UPDATE_INTERVAL}</integer>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>${WORK_DIR}/auto-update.log</string>
    <key>StandardErrorPath</key>
    <string>${WORK_DIR}/auto-update.error.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>
</dict>
</plist>
EOF

    # Set permissions (owned by user, not root)
    chown "$USER" "$AUTO_UPDATE_PLIST_PATH"
    chmod 644 "$AUTO_UPDATE_PLIST_PATH"
    
    # Load auto-update service
    echo "Starting auto-update service..."
    sudo -u "$USER" launchctl load "$AUTO_UPDATE_PLIST_PATH" 2>/dev/null || sudo -u "$USER" launchctl load -w "$AUTO_UPDATE_PLIST_PATH"
    
    echo -e "${GREEN}Auto-update service installed (checks every 15 minutes)${NC}"
else
    # If auto-update is disabled, make sure to remove any existing auto-update service
    if [ -f "$AUTO_UPDATE_PLIST_PATH" ]; then
        echo "Removing auto-update service (disabled)..."
        if sudo -u "$USER" launchctl list 2>/dev/null | grep -q "$AUTO_UPDATE_SERVICE_NAME"; then
            sudo -u "$USER" launchctl unload "$AUTO_UPDATE_PLIST_PATH" 2>/dev/null || true
        fi
        rm -f "$AUTO_UPDATE_PLIST_PATH"
    fi
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
if [ "$ENABLE_AUTO_UPDATE" = true ]; then
    echo "  Auto-update: Enabled (every 15 min)"
    echo "  Auto-update logs: ${WORK_DIR}/auto-update.log"
fi
echo ""
echo "Service management (no sudo needed):"
echo "  Start:     launchctl load ${PLIST_PATH}"
echo "  Stop:      launchctl unload ${PLIST_PATH}"
echo "  Restart:   launchctl unload ${PLIST_PATH} && launchctl load ${PLIST_PATH}"
echo "  Status:    launchctl list | grep ${SERVICE_NAME}"
echo "  Logs:      tail -f ${WORK_DIR}/vibe-kanban.log"
if [ "$ENABLE_AUTO_UPDATE" = true ]; then
    echo ""
    echo "Auto-update management:"
    echo "  Stop:      launchctl unload ${AUTO_UPDATE_PLIST_PATH}"
    echo "  Start:     launchctl load ${AUTO_UPDATE_PLIST_PATH}"
    echo "  Status:    launchctl list | grep ${AUTO_UPDATE_SERVICE_NAME}"
    echo "  Logs:      tail -f ${WORK_DIR}/auto-update.log"
fi
echo ""
if [ -n "$BACKUP_PATH" ]; then
    echo "Rollback:    sudo mv ${BACKUP_PATH} ${INSTALL_DIR}"
    echo ""
fi
if [ "$ENABLE_AUTO_UPDATE" = true ]; then
    echo -e "${CYAN}Auto-update is enabled. The service will automatically update when changes are pushed to git.${NC}"
else
    echo "To update in the future, rebuild and run this script again:"
    echo "  ./local-build.sh && sudo ./install-macos-service.sh"
fi
echo ""
echo -e "${CYAN}Note: LaunchAgents run in your user session with keychain access.${NC}"
echo -e "${CYAN}The service will start automatically when you log in.${NC}"
echo -e "${CYAN}Tip: Logs can grow large. Consider setting up log rotation with newsyslog or logrotate.${NC}"
