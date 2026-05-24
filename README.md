# Email MCP Server

[![Crates.io](https://img.shields.io/crates/v/mcp-email.svg)](https://crates.io/crates/mcp-email)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![ADK-Rust Enterprise](https://img.shields.io/badge/ADK--Rust-Enterprise-purple.svg)](https://enterprise.adk-rust.com)
[![Registry Ready](https://img.shields.io/badge/ADK_Registry-Ready-green.svg)](https://www.zavora.ai)

Multi-backend email for AI agents. Send via **SMTP**, **AWS SES**, **SendGrid**, **Gmail**, or **Microsoft Graph** — read via Gmail or Microsoft Graph. 9 tools with enterprise governance and risk classification.

## Architecture

<p align="center">
  <img src="https://raw.githubusercontent.com/zavora-ai/mcp-email/main/docs/assets/architecture.svg" alt="MCP Email Architecture" width="800"/>
</p>

## Key Principles

- **Multi-backend sending** — configure once, switch providers without code changes
- **Separation of send/read** — use SMTP or SendGrid for sending while reading from Gmail
- **No credential exposure** — tokens stay in env vars, never reach LLM context
- **HTML support** — send rich HTML emails alongside plain text
- **Registry-ready** — ships with `mcp-server.toml` for ADK-Rust Enterprise onboarding

## Tools (9)

| Tool | Purpose | Risk Class |
|------|---------|------------|
| `send_email` | Send email (plain text + HTML) via configured backend | External write |
| `list_inbox` | List inbox messages | Read-only |
| `get_email` | Get a specific email by ID | Read-only |
| `search_emails` | Search emails by query | Read-only |
| `reply_to_email` | Reply to an email | External write |
| `list_labels` | List email labels/folders | Read-only |
| `move_to_folder` | Move email to a folder/label | Internal write |
| `mark_read` | Mark an email as read | Internal write |
| `get_attachments` | Get attachment metadata for an email | Read-only |

## Send Backends

| Backend | Env Vars | Use Case |
|---------|----------|----------|
| **SMTP** (default) | `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` | Any SMTP relay (Postfix, Mailgun, Zoho, etc.) |
| **SendGrid** | `SENDGRID_API_KEY`, `SENDGRID_FROM` | High-volume transactional email |
| **AWS SES** | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`, `SES_FROM` | AWS-native workloads |
| **Gmail API** | `GMAIL_ACCESS_TOKEN` | Google Workspace users |
| **Microsoft Graph** | `MS_GRAPH_TOKEN` | Microsoft 365 users |

### Backend Priority

If multiple env vars are set, the server picks the first match:

```
SMTP → SendGrid → AWS SES → Gmail → Microsoft
```

## Read Backends

| Backend | Env Vars | Capabilities |
|---------|----------|-------------|
| **IMAP** (default) | `IMAP_HOST`, `IMAP_PORT`, `IMAP_USERNAME`, `IMAP_PASSWORD` | Inbox, search, flags — works with any email provider |
| **Gmail API** | `GMAIL_ACCESS_TOKEN` | Inbox, search, labels, attachments |
| **Microsoft Graph** | `MS_GRAPH_TOKEN` | Inbox, search, folders, attachments |

> **IMAP works with any provider** — Gmail, Outlook, Yahoo, Fastmail, Zoho, self-hosted. No OAuth needed, just host/user/password.

## Installation

```bash
cargo install mcp-email
```

Or build from source:

```bash
git clone https://github.com/zavora-ai/mcp-email
cd mcp-email
cargo build --release
```

## Configuration

### SMTP (works with any provider)

```bash
export SMTP_HOST="smtp.gmail.com"
export SMTP_PORT="587"
export SMTP_USERNAME="you@gmail.com"
export SMTP_PASSWORD="app-password"
export SMTP_FROM="you@gmail.com"
```

### IMAP (read inbox — works with any provider)

```bash
export IMAP_HOST="imap.gmail.com"
export IMAP_PORT="993"
export IMAP_USERNAME="you@gmail.com"
export IMAP_PASSWORD="app-password"
```

Common IMAP hosts:
- Gmail: `imap.gmail.com:993`
- Outlook: `outlook.office365.com:993`
- Yahoo: `imap.mail.yahoo.com:993`
- Fastmail: `imap.fastmail.com:993`
- Zoho: `imap.zoho.com:993`

### SendGrid

```bash
export SENDGRID_API_KEY="SG.xxxx"
export SENDGRID_FROM="noreply@yourdomain.com"
```

### AWS SES

```bash
export AWS_ACCESS_KEY_ID="AKIA..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_REGION="us-east-1"
export SES_FROM="noreply@yourdomain.com"
```

### Gmail (send + read)

```bash
export GMAIL_ACCESS_TOKEN="ya29.xxxx"
```

### Microsoft Graph (send + read)

```bash
export MS_GRAPH_TOKEN="eyJ0..."
```

### Combined: SendGrid for sending + Gmail for reading

```bash
export SENDGRID_API_KEY="SG.xxxx"
export SENDGRID_FROM="noreply@company.com"
export GMAIL_ACCESS_TOKEN="ya29.xxxx"  # enables read tools
```

## Client Configuration

### Claude Desktop

```json
{
  "mcpServers": {
    "email": {
      "command": "mcp-email",
      "args": [],
      "env": {
        "SMTP_HOST": "smtp.gmail.com",
        "SMTP_PORT": "587",
        "SMTP_USERNAME": "you@gmail.com",
        "SMTP_PASSWORD": "app-password",
        "SMTP_FROM": "you@gmail.com",
        "GMAIL_ACCESS_TOKEN": "ya29.xxxx"
      }
    }
  }
}
```

### Kiro

Add to `.kiro/settings/mcp.json`:

```json
{
  "mcpServers": {
    "email": {
      "command": "mcp-email",
      "args": [],
      "env": {
        "SENDGRID_API_KEY": "SG.xxxx",
        "SENDGRID_FROM": "noreply@company.com"
      }
    }
  }
}
```

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "email": {
      "command": "mcp-email",
      "args": [],
      "env": {
        "SENDGRID_API_KEY": "SG.xxxx",
        "SENDGRID_FROM": "noreply@company.com"
      }
    }
  }
}
```

## Usage Examples

### Send an email
```
"Send an email to james@company.com about the deployment being ready"
→ calls send_email with to, subject, body
```

### Search and reply
```
"Find the email from Sarah about the budget and reply saying I approve"
→ calls search_emails → reply_to_email
```

### Organize inbox
```
"Move all unread emails from marketing to the Promotions folder"
→ calls list_inbox → move_to_folder
```

## MCP Server Manifest

```toml
server_id = "mcp_email"
display_name = "Email"
version = "1.1.0"
domain = "collaboration"
risk_level = "medium"
writes_allowed = "gated"
transports = ["stdio"]
credentials = ["vault://email-credentials"]
```

## Roadmap

- [ ] IMAP read backend (generic inbox access without OAuth)
- [ ] Attachment download (content, not just metadata)
- [ ] Email templates (Handlebars/Tera)
- [ ] Batch send (multiple recipients)
- [ ] Webhook support for incoming email (SendGrid Inbound Parse)

## Registry Compliance

This server implements the [ADK MCP SDK](https://crates.io/crates/adk-mcp-sdk) contract:

- **HealthCheck** — async health probe for registry monitoring
- **mcp-server.toml** — manifest declaring tools, risk classes, and credentials
- **Structured tracing** — `RUST_LOG` env-filter for observability

## Contributors

<!-- ALL-CONTRIBUTORS-LIST:START -->
| [<img src="https://github.com/jkmaina.png" width="80px;" alt=""/><br /><sub><b>James Karanja Maina</b></sub>](https://github.com/jkmaina) |
|:---:|
<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.

---

Part of the [ADK-Rust Enterprise](https://enterprise.adk-rust.com) MCP server ecosystem.

Built with ❤️ by [Zavora AI](https://zavora.ai)
