# SMCP (Seaavey Model Context Protocol Suite)

A modular, high-performance Model Context Protocol (MCP) server suite built in Rust. Designed specifically for AI agent environments (Hermes Agent, Claude Desktop, Cursor, Codex) to eliminate runtime scripting overhead and deliver deterministic native tools.

## Architecture & Design Goals

- **Modular Domain Hierarchy:** Subdivided into logical domain modules (`src/servers/bitwarden`, `src/servers/workspace/gmail`).
- **Autonomous & In-Band MCP Setup:** Credentials can be configured **interactively via CLI** (`smcp setup ...`) OR **directly through MCP tool calls** (`gmail_set_credentials`, `bitwarden_set_password`) by an AI agent mid-conversation with pre-save TLS verification.
- **Single Canonical Config Paths:** Canonical locations (`~/.config/credentials/workspace-google` and `~/.config/credentials/bitwarden_master_password`).
- **Direct CLI Subcommand Runner:** Test and use any capability directly via `smcp call ...` without starting an MCP server daemon or using Python.
- **Zero Hardcoding:** Works out-of-the-box for any user. Sender display names are dynamically discovered directly from the server.

---

## Direct CLI Usage (`smcp call`)

```bash
# Check emails across mailboxes (inbox, spam, trash, sent, drafts, all, starred)
smcp call gmail-check --folder spam --filter all --limit 5
smcp call gmail-check --folder inbox --filter all --query Bitwarden
smcp call gmail-check --limit 10 --filter unread

# Read an email by sequence ID (with optional folder)
smcp call gmail-read 964 --folder inbox
smcp call gmail-read 5 --folder spam

# Send an email
smcp call gmail-send --to someone@example.com --subject "Subject" --body "Message text"

# Bitwarden items & passwords
smcp call bw-list --query crowdgen
smcp call bw-password CrowdGen
smcp call bw-totp CrowdGen
```

---

## Included MCP Servers

### 1. Google Workspace (`smcp workspace gmail`)

Native Rust IMAP/SMTP client supporting full mailbox traversal (`INBOX`, `[Gmail]/Spam`, `[Gmail]/Trash`, `[Gmail]/Sent Mail`, `[Gmail]/Drafts`, `[Gmail]/Starred`, `[Gmail]/Important`).

#### Registered Tools

- `gmail_set_credentials`
  - Description: Set and save Gmail credentials (email and App Password) directly via MCP, testing the connection immediately.
- `gmail_check_emails`
  - Description: Check recent emails across mailboxes with folder, filter, and keyword search options.
  - Parameters:
    - `limit` *(optional, uint, default: 10, max: 30)*
    - `filter` *(optional, string)*: `'unread'` (default), `'all'`, `'read'`, `'starred'`
    - `folder` *(optional, string)*: `'inbox'` (default), `'spam'`, `'trash'`, `'sent'`, `'drafts'`, `'all'`, `'starred'`, `'important'`
    - `query` *(optional, string)*: Keyword search query
- `gmail_read_email`
  - Description: Read full parsed body and headers of an email by sequence ID and mailbox folder.
  - Parameters:
    - `id` *(required, uint)*
    - `folder` *(optional, string, default: 'inbox')*
- `gmail_send_email`
  - Description: Send text emails via Gmail SMTP relay (`smtp.gmail.com:587`).

---

### 2. Bitwarden (`smcp bitwarden serve`)

Integrates with the local Bitwarden CLI (`bw`) through stdio transport. Transparently manages session unlocking and caching.

#### Registered Tools

- `bitwarden_set_password`: Set and save Bitwarden Master Password directly via MCP, testing unlock immediately.
- `bitwarden_status`: Check Bitwarden vault status, user email, and last sync timestamp.
- `bitwarden_sync`: Sync local vault cache with remote Bitwarden servers.
- `bitwarden_list_items`: Retrieve sanitized list of vault items (ID, Name, Type, Username). Optional search query filter.
- `bitwarden_get_item`: Get complete JSON metadata for a specific vault item by name or ID.
- `bitwarden_get_password`: Directly retrieve item password without parsing raw payload.
- `bitwarden_get_totp`: Generate live 2FA TOTP code for a designated vault item.

---

## Credential Setup

### Option A: Direct In-Band MCP Tools (Zero Terminal needed)
- `gmail_set_credentials(email, app_password)`
- `bitwarden_set_password(master_password)`

### Option B: Interactive CLI Wizard
```bash
smcp setup gmail
smcp setup bitwarden
```

---

## Installation & Build

```bash
git clone https://github.com/seaavey/smcp.git
cd smcp
cargo build --release
sudo cp target/release/smcp /usr/local/bin/smcp
```

---

## Agent Integration

### Hermes Agent
```bash
hermes mcp add bitwarden --command smcp --args bitwarden serve
hermes mcp add gmail --command smcp --args workspace gmail
```

### Claude Desktop / Cursor
```json
{
  "mcpServers": {
    "bitwarden": {
      "command": "smcp",
      "args": ["bitwarden", "serve"]
    },
    "gmail": {
      "command": "smcp",
      "args": ["workspace", "gmail"]
    }
  }
}
```

---

## License

MIT License. Copyright (c) 2026 Muhammad Adriansyah (Seaavey).
