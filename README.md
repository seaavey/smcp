# SMCP (Seaavey Model Context Protocol Suite)

A modular, high-performance Model Context Protocol (MCP) server suite built in Rust. Designed specifically for AI agent environments (Hermes Agent, Claude Desktop, Cursor, Codex) to eliminate runtime scripting overhead and deliver deterministic native tools.

## Architecture & Design Goals

- **Modular Domain Hierarchy:** Subdivided into logical domain modules (`src/servers/bitwarden`, `src/servers/workspace/gmail`).
- **Interactive Setup Wizards:** Includes `smcp setup <gmail|bitwarden>` to easily set and verify credentials with instant connectivity tests.
- **Zero Hardcoding:** Works out-of-the-box for any user via standard environment variables or standard config paths. Display names and identity are dynamically discovered directly from the server.
- **Zero Interpreter Overhead:** Native Rust executable using Tokio and official MCP SDK (`rmcp`), minimizing memory footprint and process spawn latency.

---

## Interactive Credential Setup

SMCP includes built-in setup wizards that prompt for credentials, store them securely in `~/.config/credentials/`, and instantly test authentication.

```bash
# Setup & test Gmail credentials (App Password)
smcp setup gmail

# Setup & test Bitwarden Master Password
smcp setup bitwarden
```

---

## Included MCP Servers

### 1. Bitwarden (`smcp bitwarden serve`)

Integrates with the local Bitwarden CLI (`bw`) through stdio transport. Transparently manages session unlocking and caching.

#### Configuration
- Interactive: `smcp setup bitwarden`
- Or Environment: `BW_PASSWORD` or `BITWARDEN_MASTER_PASSWORD`
- Or Config File: `~/.config/credentials/bitwarden_master_password`

#### Registered Tools

- `bitwarden_status`: Check Bitwarden vault status, user email, and last sync timestamp.
- `bitwarden_sync`: Sync local vault cache with remote Bitwarden servers.
- `bitwarden_list_items`: Retrieve sanitized list of vault items (ID, Name, Type, Username). Optional search query filter.
- `bitwarden_get_item`: Get complete JSON metadata for a specific vault item by name or ID.
- `bitwarden_get_password`: Directly retrieve item password without parsing raw payload.
- `bitwarden_get_totp`: Generate live 2FA TOTP code for a designated vault item.

---

### 2. Google Workspace (`smcp workspace gmail`)

Direct IMAP (TLS) and SMTP (TLS) client built in native Rust without external Python or browser dependencies.

#### Account Setup for Gmail:
1. Enable **2-Step Verification** on your Google Account:
   https://myaccount.google.com/signinoptions/two-step-verification
2. Generate a 16-character **App Password** for 'Mail':
   https://myaccount.google.com/apppasswords
3. Run the setup wizard:
   ```bash
   smcp setup gmail
   ```
   *(Or set `GMAIL_EMAIL` and `GMAIL_APP_PASSWORD` environment variables / file).*

Sender display names are automatically detected from the authenticated user's sent mailbox on the IMAP server.

#### Registered Tools

- `gmail_check_emails`
  - Description: Check recent INBOX emails (returns numeric sequence ID, Date, Sender, and Subject).
  - Parameters:
    - `limit` *(optional, uint, default: 10, max: 30)*
    - `filter` *(optional, string)*: `'unread'` (default) or `'all'`
- `gmail_read_email`
  - Description: Read full parsed body and headers of an email by sequence ID.
  - Parameters:
    - `id` *(required, uint)*: Sequence ID from `gmail_check_emails`
- `gmail_send_email`
  - Description: Send text emails via Gmail SMTP relay (`smtp.gmail.com:587`).
  - Parameters:
    - `to` *(required, string)*: Recipient email
    - `subject` *(required, string)*: Subject line
    - `body` *(required, string)*: Email body
    - `from_name` *(optional, string)*: Custom sender display name (defaults to auto-detected server name)

---

## Installation & Build

### Prerequisites

- Rust 1.80+ (`cargo`, `rustc`)
- OpenSSL development libraries (`libssl-dev`)
- Bitwarden CLI (`npm install -g @bitwarden/cli`)

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
# Bitwarden
hermes mcp add bitwarden --command smcp --args bitwarden serve

# Google Workspace / Gmail
hermes mcp add gmail --command smcp --args workspace gmail
```

Or add directly to `~/.hermes/config.yaml`:

```yaml
mcp_servers:
  bitwarden:
    command: "smcp"
    args: ["bitwarden", "serve"]
  gmail:
    command: "smcp"
    args: ["workspace", "gmail"]
```

Test connections:

```bash
hermes mcp test bitwarden
hermes mcp test gmail
```

---

## License

MIT License. Copyright (c) 2026 Muhammad Adriansyah (Seaavey).
