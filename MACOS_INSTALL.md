# macOS Server Installation

Build and run Vibe Kanban as a background service on macOS.

## Quick Start

```bash
# 1. Install prerequisites (if needed)
brew install node
npm install -g pnpm
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Build
pnpm i
pnpm run generate-types
./local-build.sh

# 3. Install & start service
sudo ./install-macos-service.sh
```

Done! Access at **http://localhost:3000**

---

## Prerequisites

| Requirement | Install Command |
|-------------|-----------------|
| Rust (stable) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node.js (>=18) | `brew install node` |
| pnpm (>=8) | `npm install -g pnpm` |
| Git | `brew install git` |

---

## Installation

### Build the Project

```bash
pnpm i                      # Install dependencies
pnpm run generate-types     # Generate TypeScript types
./local-build.sh            # Build binaries
```

### Install the Service

```bash
sudo ./install-macos-service.sh
```

The script automatically:
- Installs binary to `/usr/local/bin/vibe-kanban`
- Creates a LaunchDaemon (auto-starts on boot)
- Starts the service immediately
- Verifies the service is healthy

### Custom Port

```bash
PORT=8080 sudo ./install-macos-service.sh
```

---

## Updating

Same script handles updates — just rebuild and run:

```bash
git pull                              # Get latest code
pnpm i                                # Update dependencies (if needed)
./local-build.sh                      # Rebuild
sudo ./install-macos-service.sh       # Update service
```

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
sudo launchctl unload /Library/LaunchDaemons/com.vibekanban.server.plist
sudo launchctl load /Library/LaunchDaemons/com.vibekanban.server.plist
```

---

## Service Management

| Action | Command |
|--------|---------|
| Start | `sudo launchctl load /Library/LaunchDaemons/com.vibekanban.server.plist` |
| Stop | `sudo launchctl unload /Library/LaunchDaemons/com.vibekanban.server.plist` |
| Status | `sudo launchctl list \| grep vibekanban` |
| Logs | `tail -f /var/vibe-kanban/vibe-kanban.log` |

---

## File Locations

| What | Where |
|------|-------|
| Binary | `/usr/local/bin/vibe-kanban` |
| MCP Binary | `/usr/local/bin/vibe-kanban-mcp` |
| Service Config | `/Library/LaunchDaemons/com.vibekanban.server.plist` |
| Logs | `/var/vibe-kanban/vibe-kanban.log` |
| Database & Config | `~/Library/Application Support/ai.bloop.vibe-kanban/` |

> **Note:** Updates only replace the binary. Your database and settings are preserved.

---

## Configuration

### Change Port or Host

Option 1: Set during install
```bash
PORT=8080 HOST=0.0.0.0 sudo ./install-macos-service.sh --force
```

Option 2: Edit plist directly
```bash
sudo nano /Library/LaunchDaemons/com.vibekanban.server.plist
# Then restart:
sudo launchctl unload /Library/LaunchDaemons/com.vibekanban.server.plist
sudo launchctl load /Library/LaunchDaemons/com.vibekanban.server.plist
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port |
| `HOST` | `0.0.0.0` | Server host |
| `RUST_LOG` | `info` | Log level |

---

## Uninstall

```bash
sudo ./uninstall-macos-service.sh
```

Or manually:
```bash
sudo launchctl unload /Library/LaunchDaemons/com.vibekanban.server.plist
sudo rm -f /Library/LaunchDaemons/com.vibekanban.server.plist
sudo rm -f /usr/local/bin/vibe-kanban
sudo rm -f /usr/local/bin/vibe-kanban-mcp
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

# Reinstall with different port
PORT=8080 sudo ./install-macos-service.sh --force
```

### Permission issues

```bash
sudo chown -R $(whoami) /var/vibe-kanban
sudo chown -R $(whoami) ~/Library/Application\ Support/ai.bloop.vibe-kanban
```

---

## Script Options

```bash
sudo ./install-macos-service.sh [OPTIONS]

Options:
  --force     Force reinstall (recreates service config)
  --no-mcp    Skip MCP binary installation
  --help      Show help
```

---

## Remote Access

The service binds to `0.0.0.0` by default, allowing remote connections.

1. Open firewall: `sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /usr/local/bin/vibe-kanban`
2. Access: `http://YOUR_SERVER_IP:3000`

For production, consider using a reverse proxy (nginx/Caddy) with SSL.
