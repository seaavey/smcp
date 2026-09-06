# SMCP (Seaavey Model Context Protocol Suite)

A modular, high-performance Model Context Protocol (MCP) server suite built in Rust. Designed specifically for AI agent environments (Hermes Agent, Claude Desktop, Cursor, Codex) to eliminate runtime scripting overhead and deliver deterministic native tools.

## Architecture & Design Goals

- **Single Binary Multiplexer:** Unified CLI entry point running distinct MCP servers via subcommands (`smcp <subcommand>`).
- **Zero Interpreter Overhead:** Native Rust executable using Tokio and official MCP SDK (`rmcp`), minimizing memory footprint and process spawn latency.
- **Agent Self-Sufficient:** Native credential resolution and automated session lifecycle handling without manual terminal intervention.

---

## Included MCP Servers

### 1. Bitwarden (`smcp bitwarden`)

Integrates with the local Bitwarden CLI (`bw`) through stdio transport. It transparently manages vault unlock state, caching session keys in memory while resolving credentials automatically from secure configuration files.

#### Registered Tools

- `bitwarden_status`
  - Description: Check Bitwarden vault status, user email, and last sync timestamp.
  - Parameters: None

- `bitwarden_sync`
  - Description: Sync local vault cache with remote Bitwarden servers.
  - Parameters: None

- `bitwarden_list_items`
  - Description: Retrieve sanitized list of vault items (ID, Name, Type, Username).
  - Parameters:
    - `query` *(optional, string)*: Filter items by search keyword.

- `bitwarden_get_item`
  - Description: Get complete JSON metadata for a specific vault item.
  - Parameters:
    - `id_or_name` *(required, string)*: Item ID or exact item name.

- `bitwarden_get_password`
  - Description: Directly retrieve item password without parsing raw payload.
  - Parameters:
    - `id_or_name` *(required, string)*: Item ID or exact item name.

- `bitwarden_get_totp`
  - Description: Generate live 2FA TOTP code for a designated vault item.
  - Parameters:
    - `id_or_name` *(required, string)*: Item ID or exact item name.

---

## Installation & Build

### Prerequisites

- Rust 1.80+ (`cargo`, `rustc`)
- Bitwarden CLI (`npm install -g @bitwarden/cli` or binary install)

### Build Release Binary

```bash
git clone https://github.com/seaavey/smcp.git
cd smcp
cargo build --release
sudo cp target/release/smcp /usr/local/bin/smcp
```

Verify the installation:

```bash
smcp --help
```

---

## Agent Integration

### Hermes Agent

Register using the Hermes MCP command:

```bash
hermes mcp add bitwarden --command smcp --args bitwarden
```

Or add directly to `~/.hermes/config.yaml`:

```yaml
mcp_servers:
  bitwarden:
    command: "smcp"
    args: ["bitwarden"]
```

Test connection:

```bash
hermes mcp test bitwarden
```

### Claude Desktop / Cursor

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "bitwarden": {
      "command": "smcp",
      "args": ["bitwarden"]
    }
  }
}
```

---

## License

MIT License. Copyright (c) 2026 Muhammad Adriansyah (Seaavey).
