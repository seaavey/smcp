# SMCP (Seaavey MCP Suite)

High-performance Model Context Protocol (MCP) server suite written in Rust for Hermes Agent and MCP clients.

## Servers Included

### 1. Bitwarden (`smcp bitwarden`)
Interacts natively with the local Bitwarden CLI (`bw`), handles session unlock transparently using credentials in `~/.config/credentials/bitwarden_master_password`.

#### Available Tools:
- `bitwarden_status`: Check Bitwarden status and vault sync status.
- `bitwarden_sync`: Sync Bitwarden local cache with remote vault.
- `bitwarden_list_items`: List vault items with ID, Name, Type, and Username. Accepts optional `query` string.
- `bitwarden_get_item`: Get full details of an item by name or ID.
- `bitwarden_get_password`: Retrieve password directly for an item.
- `bitwarden_get_totp`: Generate active 2FA TOTP code for an item.

## Building & Installing

```bash
cargo build --release
cp target/release/smcp /usr/local/bin/smcp
```

## Hermes Configuration

Add to `~/.hermes/config.yaml`:

```yaml
mcp_servers:
  bitwarden:
    command: "smcp"
    args: ["bitwarden"]
```
