# macOS Server Installation

Build and run Vibe Kanban as a background service on macOS.

> **Note:** Vibe Kanban runs as a LaunchAgent (user service) rather than a LaunchDaemon. This ensures the service has access to your login keychain, which is required for Claude Code OAuth authentication to work properly. The service starts automatically when you log in.

## Quick Start

```bash
# 1. Configure environment variables
cp .env.example .env
# Edit .env with your values (required: VK_PUBLIC_BASE_URL, GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET)

# 2. Install & start service (handles everything automatically)
sudo ./install-macos-service.sh
```

Done! Access at **http://localhost:3000**

The install script automatically handles:
- Installing Homebrew (if not present)
- Installing Node.js, pnpm, and Rust
- Installing npm dependencies (`pnpm install`)
- Generating TypeScript types
- Building the project
- Installing and starting the service
- Setting up auto-update (checks for updates every 15 minutes)

---

## Prerequisites

The install script will automatically install these if missing:

| Requirement | Installed Via |
|-------------|---------------|
| Homebrew | Automatic |
| Node.js (>=18) | `brew install node` |
| pnpm (>=8) | `npm install -g pnpm` |
| Rust (stable) | rustup |
| Git | `brew install git` |

> **Note:** If you already have all prerequisites installed, you can use `--skip-deps` to skip the dependency checks.

---

## Installation

### Configure Environment

Create your `.env` file with the required variables:

```bash
cp .env.example .env
# Edit .env with your values
```

### Run the Install Script

```bash
sudo ./install-macos-service.sh
```

The script automatically:
- Checks and installs all prerequisites (Homebrew, Node.js, pnpm, Rust)
- Installs npm dependencies with `pnpm install`
- Generates TypeScript types
- Builds the project (via `local-build.sh`)
- Installs binary to `/usr/local/bin/vibe-kanban`
- Creates a LaunchAgent (auto-starts on user login)
- Starts the service immediately
- Verifies the service is healthy
- Migrates from LaunchDaemon to LaunchAgent if needed

### Skip Dependency Installation

If you already have all prerequisites installed:

```bash
sudo ./install-macos-service.sh --skip-deps
```

### Custom Port

Set `PORT=8080` in your `.env` file, then run the install script.

---

## Auto-Update

By default, the install script sets up an auto-update daemon that checks for git updates every 15 minutes. When changes are detected, it automatically pulls and reinstalls the new version.

### How It Works

1. The `com.vibekanban.autoupdate` LaunchAgent runs every 15 minutes
2. It fetches the latest changes from the git remote
3. If updates are available, it pulls and runs the install script
4. All activity is logged to `/var/vibe-kanban/auto-update.log`

### Disable Auto-Update

To install without auto-update:

```bash
sudo ./install-macos-service.sh --no-auto-update
```

Or set in your `.env` file:

```
AUTO_UPDATE=0
```

### Auto-Update Management

| Action | Command |
|--------|---------|
| Stop | `launchctl unload ~/Library/LaunchAgents/com.vibekanban.autoupdate.plist` |
| Start | `launchctl load ~/Library/LaunchAgents/com.vibekanban.autoupdate.plist` |
| Status | `launchctl list \| grep autoupdate` |
| Logs | `tail -f /var/vibe-kanban/auto-update.log` |

---

## Manual Updating

If auto-update is disabled, you can update manually:

```bash
git pull                        # Get latest code
sudo ./install-macos-service.sh # Handles deps, build, and update
```

The script automatically detects when dependencies need updating and rebuilds as needed.

> **Note:** When updating, the script preserves your existing service configuration unless you use `--force`.

The script will:
- Stop the running service
- Backup the old binary
- Install the new binary
- Preserve your configuration
- Restart the service

### Rollback

```bash
# List backups
ls /usr/local/bin/vibe-kanban.backup.*

# Restore
sudo mv /usr/local/bin/vibe-kanban.backup.TIMESTAMP /usr/local/bin/vibe-kanban
launchctl unload ~/Library/LaunchAgents/com.vibekanban.server.plist
launchctl load ~/Library/LaunchAgents/com.vibekanban.server.plist
```

---

## Service Management

LaunchAgents run in your user session, so no `sudo` is needed for these commands:

| Action | Command |
|--------|---------|
| Start | `launchctl load ~/Library/LaunchAgents/com.vibekanban.server.plist` |
| Stop | `launchctl unload ~/Library/LaunchAgents/com.vibekanban.server.plist` |
| Status | `launchctl list \| grep vibekanban` |
| Logs | `tail -f /var/vibe-kanban/vibe-kanban.log` |

---

## File Locations

| What | Where |
|------|-------|
| Binary | `/usr/local/bin/vibe-kanban` |
| MCP Binary | `/usr/local/bin/vibe-kanban-mcp` |
| Auto-Update Script | `/usr/local/bin/vibe-kanban-autoupdate` |
| Service Config | `~/Library/LaunchAgents/com.vibekanban.server.plist` |
| Auto-Update Config | `~/Library/LaunchAgents/com.vibekanban.autoupdate.plist` |
| Logs | `/var/vibe-kanban/vibe-kanban.log` |
| Auto-Update Logs | `/var/vibe-kanban/auto-update.log` |
| Database & Config | `~/Library/Application Support/ai.bloop.vibe-kanban/` |
| Repo Path (for auto-update) | `/var/vibe-kanban/.repo-path` |

> **Note:** Updates only replace the binary. Your database and settings are preserved.

---

## Configuration

### Change Port or Host

1. Edit your `.env` file:
   ```
   PORT=8080
   HOST=0.0.0.0
   ```

2. Reinstall to apply changes:
   ```bash
   sudo ./install-macos-service.sh --force
   ```

Alternatively, edit the plist directly:
```bash
nano ~/Library/LaunchAgents/com.vibekanban.server.plist
# Then restart:
launchctl unload ~/Library/LaunchAgents/com.vibekanban.server.plist
launchctl load ~/Library/LaunchAgents/com.vibekanban.server.plist
```

### Environment Variables

All configuration is done via the `.env` file. Copy the example and edit:

```bash
cp .env.example .env
nano .env  # or your preferred editor
```

**Required:**

| Variable | Description |
|----------|-------------|
| `VK_PUBLIC_BASE_URL` | Public URL where the service is accessible (e.g., `https://example.com`) |
| `GOOGLE_CLIENT_ID` | Google OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | Google OAuth client secret |

**Optional:**

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port |
| `HOST` | `0.0.0.0` | Server host (defaults to `127.0.0.1` when `TAILSCALE_FUNNEL=1`) |
| `TAILSCALE_FUNNEL` | - | Set to `1` to enable Tailscale Funnel for public HTTPS access |
| `AUTO_UPDATE` | `1` | Set to `0` to disable the auto-update daemon |
| `RUST_LOG` | `info` | Log level |

---

## Uninstall

```bash
sudo ./uninstall-macos-service.sh
```

This removes:
- Main service and auto-update LaunchAgents
- All binaries (`vibe-kanban`, `vibe-kanban-mcp`, `vibe-kanban-autoupdate`)
- Sudoers entry for auto-update
- Optionally the working directory (`/var/vibe-kanban`)

Or manually:
```bash
# Stop services
launchctl unload ~/Library/LaunchAgents/com.vibekanban.server.plist
launchctl unload ~/Library/LaunchAgents/com.vibekanban.autoupdate.plist

# Remove plists
rm -f ~/Library/LaunchAgents/com.vibekanban.server.plist
rm -f ~/Library/LaunchAgents/com.vibekanban.autoupdate.plist

# Remove binaries
sudo rm -f /usr/local/bin/vibe-kanban
sudo rm -f /usr/local/bin/vibe-kanban-mcp
sudo rm -f /usr/local/bin/vibe-kanban-autoupdate

# Remove sudoers entry
sudo rm -f /etc/sudoers.d/vibe-kanban-autoupdate

# Remove working directory
sudo rm -rf /var/vibe-kanban
```

---

## Troubleshooting

### Service won't start

```bash
# Check error logs
cat /var/vibe-kanban/vibe-kanban.error.log

# Test binary manually
/usr/local/bin/vibe-kanban
```

### Port already in use

```bash
# Find what's using the port
lsof -i :3000

# Change PORT in .env, then reinstall
sudo ./install-macos-service.sh --force
```

### Permission issues

```bash
sudo chown -R $(whoami) /var/vibe-kanban
sudo chown -R $(whoami) ~/Library/Application\ Support/ai.bloop.vibe-kanban
```

### Auto-update not working

```bash
# Check auto-update logs
cat /var/vibe-kanban/auto-update.log

# Verify the service is running
launchctl list | grep autoupdate

# Check repo path is set
cat /var/vibe-kanban/.repo-path

# Manually trigger an update check
/usr/local/bin/vibe-kanban-autoupdate
```

Common issues:
- **Sudoers not configured**: The auto-update script needs passwordless sudo. Reinstall with `sudo ./install-macos-service.sh --force`
- **Git credentials missing**: Ensure git can pull without prompting for credentials
- **Repo path invalid**: The stored repo path may be outdated. Reinstall the service.

---

## Script Options

```bash
sudo ./install-macos-service.sh [OPTIONS]

Options:
  --force           Force reinstall (recreates service config)
  --no-mcp          Skip MCP binary installation
  --skip-deps       Skip dependency installation (use if you already have Homebrew, Node, pnpm, Rust)
  --tailscale-funnel  Enable Tailscale Funnel (or use TAILSCALE_FUNNEL=1 in .env)
  --no-auto-update  Disable auto-update daemon (or use AUTO_UPDATE=0 in .env)
  --help            Show help
```

The script automatically:
- Installs all prerequisites (Homebrew, Node.js, pnpm, Rust) if missing
- Runs `pnpm install` and `pnpm run generate-types`
- Builds the project with `local-build.sh`
- Reads configuration from `.env` in the project directory

See `.env.example` for all available environment variables.

---

## Remote Access

### Public HTTPS (Tailscale Funnel)

Tailscale Funnel provides a public HTTPS URL without port forwarding (recommended).

1. Add to your `.env`:
   ```
   TAILSCALE_FUNNEL=1
   VK_PUBLIC_BASE_URL=https://your-machine.your-tailnet.ts.net
   ```

2. Install/reinstall the service:
   ```bash
   sudo ./install-macos-service.sh --force
   ```

The script will automatically:
- Set `HOST=127.0.0.1` (localhost only, Funnel handles external traffic)
- Configure Tailscale Funnel to expose the service

**Requirements:**
- Tailscale is installed and connected (`tailscale up`)
- Your tailnet allows Funnel for this node (ACL/nodeAttrs)
- MagicDNS/HTTPS enabled for your tailnet

**Get the URL / verify status:**

```bash
sudo tailscale funnel status
```

> Funnel publishes to the public internet at a URL like `https://<machine>.<tailnet>.ts.net`.

### Direct IP (LAN/WAN)

The service binds to `0.0.0.0` by default, allowing remote connections.

1. Open firewall: `sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /usr/local/bin/vibe-kanban`
2. Access: `http://YOUR_SERVER_IP:3000`

For production, consider using a reverse proxy (nginx/Caddy) with SSL.
