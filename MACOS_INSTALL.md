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

# 3. Configure environment variables
cp .env.example .env
# Edit .env with your values (required: VK_PUBLIC_BASE_URL, VK_ANTHROPIC_API_KEY, GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET)

# 4. Install & start service (reads .env automatically)
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

First, create your `.env` file with the required variables:

```bash
cp .env.example .env
# Edit .env with your values
```

Then run the install script (it reads `.env` automatically):

```bash
sudo ./install-macos-service.sh
```

The script automatically:
- Installs binary to `/usr/local/bin/vibe-kanban`
- Creates a LaunchDaemon (auto-starts on boot)
- Starts the service immediately
- Verifies the service is healthy

### Custom Port

Set `PORT=8080` in your `.env` file, then run the install script.

---

## Updating

Same script handles updates — just rebuild and run:

```bash
git pull                        # Get latest code
pnpm i                          # Update dependencies (if needed)
./local-build.sh                # Rebuild
sudo ./install-macos-service.sh # Update service
```

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
sudo nano /Library/LaunchDaemons/com.vibekanban.server.plist
# Then restart:
sudo launchctl unload /Library/LaunchDaemons/com.vibekanban.server.plist
sudo launchctl load /Library/LaunchDaemons/com.vibekanban.server.plist
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
| `VK_ANTHROPIC_API_KEY` | Anthropic API key for AI features |
| `GOOGLE_CLIENT_ID` | Google OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | Google OAuth client secret |

**Optional:**

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port |
| `HOST` | `0.0.0.0` | Server host (defaults to `127.0.0.1` when `TAILSCALE_FUNNEL=1`) |
| `TAILSCALE_FUNNEL` | - | Set to `1` to enable Tailscale Funnel for public HTTPS access |
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

# Change PORT in .env, then reinstall
sudo ./install-macos-service.sh --force
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
  --tailscale-funnel  Enable Tailscale Funnel (or use TAILSCALE_FUNNEL=1 in .env)
  --help      Show help
```

The script automatically reads from `.env` in the project directory. See `.env.example` for all available variables.

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
