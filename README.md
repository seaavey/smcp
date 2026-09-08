# SMCP (Seaavey Model Context Protocol Suite)

A modular, high-performance Model Context Protocol (MCP) server suite built in Rust. Designed specifically for AI agent environments (Hermes Agent, Claude Desktop, Cursor, Codex) to eliminate runtime scripting overhead and deliver deterministic native tools.

## Architecture & Design Goals

- **Modular Domain Hierarchy:** Subdivided into logical domain modules (`src/servers/bitwarden`, `src/servers/workspace/gmail`).
- **Autonomous & In-Band MCP Setup:** Credentials can be configured **interactively via CLI** (`smcp setup ...`) OR **directly through MCP tool calls** (`gmail_set_credentials`, `bitwarden_set_password`) by an AI agent mid-conversation with pre-save TLS/unlock verification.
- **Permanent UID Email Model:** Gmail uses permanent IMAP UIDs (Unique Identifiers) rather than ephemeral sequence numbers, ensuring robust actions and threading.
- **Full Email Lifecycle:** Read, search, send (HTML/text), auto-threaded reply (`In-Reply-To`/`References`), attachment metadata, and email management (`trash`, `star`, `mark_read`).
- **Bitwarden Vault Mutation:** Create login credentials and generate secure random passwords directly from the suite.
- **Direct CLI Subcommand Runner:** Test and use any capability directly via `smcp call ...` without starting an MCP server daemon or using Python.

---

## Direct CLI Usage (`smcp call`)

```bash
# Check emails across mailboxes (inbox, spam, trash, sent, drafts, all, starred)
smcp call gmail-check --folder spam --filter all --limit 5
smcp call gmail-check --folder inbox --filter all --query Bitwarden
smcp call gmail-check --limit 10 --filter unread

# Read an email by permanent UID (with attachments & header metadata)
smcp call gmail-read 5301 --folder inbox

# Reply to an email (automatically tracks subject and In-Reply-To thread)
smcp call gmail-reply 5301 --body "Thank you, verified!"

# Manage emails (trash, mark_read, mark_unread, star, unstar)
smcp call gmail-manage 5301 trash

# Send an email with an optional local attachment
smcp call gmail-send --to someone@example.com --subject "Subject" --body "See attached file" --attachment /path/to/file.pdf

# Send an email (plain text or HTML)
smcp call gmail-send --to someone@example.com --subject "Subject" --body "<h1>Hello</h1>" --is-html

# Bitwarden items & passwords
smcp call bw-list --query crowdgen
smcp call bw-password CrowdGen
smcp call bw-totp CrowdGen
smcp call bw-generate --length 24
```

---

## Included MCP Servers

### 1. Google Workspace (`smcp workspace gmail`)

Native Rust IMAP/SMTP client supporting full mailbox traversal (`INBOX`, `[Gmail]/Spam`, `[Gmail]/Trash`, `[Gmail]/Sent Mail`, `[Gmail]/Drafts`, `[Gmail]/Starred`, `[Gmail]/Important`).

#### Registered Tools

- `gmail_set_credentials`: Set and save Gmail credentials directly via MCP, testing the TLS connection immediately before saving.
- `gmail_check_emails`: Check recent emails with permanent `uid`, filter (`unread`, `all`, `read`, `starred`), folder, and search query.
- `gmail_read_email`: Read complete content, attachment metadata (`filename`, `content_type`, `size`), sender/receiver headers, and body by `uid`.
- `gmail_reply_email`: Reply to an existing email thread using `uid` (automatically extracts sender, injects `In-Reply-To`, `References`, and `Re:` subject).
- `gmail_manage_email`: Execute inbox triage actions (`mark_read`, `mark_unread`, `star`, `unstar`, `trash`) by `uid`.
- `gmail_send_email`: Send emails via Gmail SMTP relay (supports plain text, HTML, and local file attachments).

---

### 2. Bitwarden (`smcp bitwarden serve`)

Integrates with the local Bitwarden CLI (`bw`) through stdio transport. Transparently manages session unlocking, caching, and item creation.

#### Registered Tools

- `bitwarden_set_password`: Set and save Bitwarden Master Password directly via MCP, testing unlock immediately.
- `bitwarden_generate_password`: Generate cryptographically secure passwords via Bitwarden CLI options (`length`, `special`, `numbers`, `uppercase`, `lowercase`).
- `bitwarden_create_login_item`: Create and store new login credentials into the vault (`name`, `username`, `password`, `uri`, `notes`).
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
