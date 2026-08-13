# Email MCP Server

[![Crates.io](https://img.shields.io/crates/v/mcp-email.svg)](https://crates.io/crates/mcp-email)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![ADK-Rust Enterprise](https://img.shields.io/badge/ADK--Rust-Enterprise-purple.svg)](https://enterprise.adk-rust.com)
[![Registry Ready](https://img.shields.io/badge/ADK_Registry-Ready-green.svg)](https://www.zavora.ai)

The most complete multi-backend email MCP server. **24 tools** across **5 send backends** and **3 read backends** — send via SMTP, AWS SES, or SendGrid while reading from any IMAP server. File attachments, OAuth with auto-refresh, single Rust binary with enterprise governance.

## Architecture

<p align="center">
  <img src="https://raw.githubusercontent.com/zavora-ai/mcp-email/main/docs/assets/architecture.svg" alt="MCP Email Architecture" width="800"/>
</p>

## Key Principles

- **Multi-backend** — 5 send + 3 read backends, mix and match freely
- **IMAP-first reading** — works with any email provider, no OAuth required
- **SMTP-first sending** — universal, works with any relay
- **Full email lifecycle** — send, read, reply, forward, draft, organize, batch, delete
- **No credential exposure** — tokens stay in env vars, never reach LLM context
- **Single binary** — no Node.js, no Python, no runtime dependencies

## Comparison with Other Email MCP Servers

| Feature | marlinjai/email-mcp | GongRzhe/Gmail-MCP | **Ours** |
|---------|:---:|:---:|:---:|
| Send email | ✅ | ✅ | ✅ |
| CC/BCC | ✅ | ✅ | ✅ |
| HTML email | ✅ | ✅ | ✅ |
| Reply | ✅ (reply-all) | ❌ | ✅ |
| Forward | ✅ | ❌ | ✅ |
| Drafts (create/list/send) | ✅ | ✅ | ✅ |
| Attachments send | ✅ | ✅ | ✅ |
| Attachments download | ✅ | ✅ | ✅ (base64) |
| Threads/conversations | ✅ | ❌ | ✅ |
| Full body retrieval | ✅ | ✅ | ✅ |
| Search | ✅ | ✅ | ✅ |
| Mark read/unread | ✅ | ✅ | ✅ |
| Star/flag | ✅ | ❌ | ✅ |
| Move to folder | ✅ | ✅ | ✅ |
| Delete (trash/permanent) | ✅ | ✅ | ✅ |
| Labels (create/delete) | ✅ | ✅ | ✅ |
| Batch operations | ✅ | ✅ | ✅ |
| Filters | ❌ | ✅ | ❌ |
| Multi-account | ✅ | ❌ | ❌ |
| **SMTP send** | ✅ | ❌ | ✅ |
| **IMAP read** | ✅ | ❌ | ✅ |
| **AWS SES** | ❌ | ❌ | ✅ |
| **SendGrid** | ❌ | ❌ | ✅ |
| **Registry governance** | ❌ | ❌ | ✅ |
| **Risk classification** | ❌ | ❌ | ✅ |
| **Rust / single binary** | ❌ (Node) | ❌ (Node) | ✅ |
| **Tools** | 24 | 18 | **24** |

## Tools (24)

### Sending & Composing (5)

| Tool | Purpose | Risk Class |
|------|---------|------------|
| `send_email` | Send email with CC/BCC, plain text + HTML | External write |
| `reply_to_email` | Reply to an email (preserves threading) | External write |
| `forward_email` | Forward an email to new recipients | External write |
| `create_draft` | Save a draft without sending | Internal write |
| `send_draft` | Send a previously saved draft | External write |

### Reading & Searching (6)

| Tool | Purpose | Risk Class |
|------|---------|------------|
| `list_inbox` | List inbox messages | Read-only |
| `get_email` | Get email metadata (subject, from, date) | Read-only |
| `get_email_body` | Get full email body (HTML or plain text) | Read-only |
| `get_thread` | Get entire email thread/conversation | Read-only |
| `search_emails` | Search by query (Gmail syntax or IMAP SEARCH) | Read-only |
| `download_attachment` | Download attachment content (base64) | Read-only |

### Organization (7)

| Tool | Purpose | Risk Class |
|------|---------|------------|
| `list_labels` | List all labels/folders | Read-only |
| `list_drafts` | List saved drafts | Read-only |
| `get_attachments` | Get attachment metadata for an email | Read-only |
| `move_to_folder` | Move email to a folder/label | Internal write |
| `mark_read` | Mark email as read | Internal write |
| `mark_unread` | Mark email as unread | Internal write |
| `star_email` | Star/flag a message | Internal write |

### Management (3)

| Tool | Purpose | Risk Class |
|------|---------|------------|
| `delete_email` | Trash or permanently delete | Destructive |
| `create_label` | Create a new label/folder | Internal write |
| `delete_label` | Delete a label/folder | Destructive |

### Batch Operations (3)

| Tool | Purpose | Risk Class |
|------|---------|------------|
| `batch_delete` | Delete multiple emails at once | Destructive |
| `batch_move` | Move multiple emails to a folder | Internal write |
| `batch_mark` | Mark multiple emails read/unread | Internal write |

## Backend Status

| Backend | Send | Read | OAuth | Tested |
|---------|:---:|:---:|:---:|:---:|
| **SMTP 465** | ✅ | — | — | ✅ |
| **SMTP 587** (STARTTLS) | ✅ | — | — | ✅ |
| **AWS SES** | ✅ | — | — | ✅ |
| **SendGrid** | ✅ | — | — | Ready |
| **Gmail API** | ✅ | ✅ | ✅ auto-refresh | ✅ |
| **Microsoft Graph** | ✅ | ✅ | ✅ auto-refresh | Ready |
| **IMAP** | — | ✅ | — | ✅ |

## Send Backends (5)

| Backend | Env Vars | Use Case |
|---------|----------|----------|
| **SMTP** (default) | `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` | Any SMTP relay — Postfix, Mailgun, Zoho, Gmail SMTP |
| **SendGrid** | `SENDGRID_API_KEY`, `SENDGRID_FROM` | High-volume transactional email |
| **AWS SES** | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`, `SES_FROM` | AWS-native workloads |
| **Gmail API** | `GMAIL_ACCESS_TOKEN` | Google Workspace (OAuth) |
| **Microsoft Graph** | `MS_GRAPH_TOKEN` | Microsoft 365 (OAuth) |

**Priority:** SMTP → SendGrid → SES → Gmail → Microsoft

## Read Backends (3)

| Backend | Env Vars | Use Case |
|---------|----------|----------|
| **IMAP** (default) | `IMAP_HOST`, `IMAP_PORT`, `IMAP_USERNAME`, `IMAP_PASSWORD` | Any provider — no OAuth needed |
| **Gmail API** | `GMAIL_ACCESS_TOKEN` | Full Gmail features (threads, labels) |
| **Microsoft Graph** | `MS_GRAPH_TOKEN` | Full Outlook features (folders, categories) |

**Priority:** IMAP → Gmail → Microsoft

### Common IMAP Hosts

| Provider | Host | Port |
|----------|------|------|
| Gmail | `imap.gmail.com` | 993 |
| Outlook | `outlook.office365.com` | 993 |
| Yahoo | `imap.mail.yahoo.com` | 993 |
| Fastmail | `imap.fastmail.com` | 993 |
| Zoho | `imap.zoho.com` | 993 |
| iCloud | `imap.mail.me.com` | 993 |
| ProtonMail | `127.0.0.1` (Bridge) | 1143 |

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

## Configuration Examples

### SMTP + IMAP (universal — works with any provider)

```bash
export SMTP_HOST="smtp.gmail.com"
export SMTP_PORT="587"
export SMTP_USERNAME="you@gmail.com"
export SMTP_PASSWORD="app-password"
export SMTP_FROM="you@gmail.com"
export IMAP_HOST="imap.gmail.com"
export IMAP_PORT="993"
export IMAP_USERNAME="you@gmail.com"
export IMAP_PASSWORD="app-password"
```

### SendGrid (send) + Gmail API (read)

```bash
export SENDGRID_API_KEY="SG.xxxx"
export SENDGRID_FROM="noreply@company.com"
export GMAIL_ACCESS_TOKEN="ya29.xxxx"
```

### AWS SES (send) + IMAP (read)

```bash
export AWS_ACCESS_KEY_ID="AKIA..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_REGION="us-east-1"
export SES_FROM="noreply@company.com"
export IMAP_HOST="imap.gmail.com"
export IMAP_PORT="993"
export IMAP_USERNAME="you@gmail.com"
export IMAP_PASSWORD="app-password"
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
        "IMAP_HOST": "imap.gmail.com",
        "IMAP_PORT": "993",
        "IMAP_USERNAME": "you@gmail.com",
        "IMAP_PASSWORD": "app-password"
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
        "SENDGRID_FROM": "noreply@company.com",
        "IMAP_HOST": "imap.gmail.com",
        "IMAP_PORT": "993",
        "IMAP_USERNAME": "you@gmail.com",
        "IMAP_PASSWORD": "app-password"
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
        "SMTP_HOST": "smtp.gmail.com",
        "SMTP_PORT": "587",
        "SMTP_USERNAME": "you@gmail.com",
        "SMTP_PASSWORD": "app-password",
        "SMTP_FROM": "you@gmail.com"
      }
    }
  }
}
```

### Windsurf

Add to `~/.codeium/windsurf/mcp_config.json`:

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

### Send an email with CC
```
"Send an email to james@company.com, CC sarah@company.com, about the deployment being ready"
→ calls send_email with to, cc, subject, body
```

### Forward an email
```
"Forward the latest email from the client to the legal team"
→ calls search_emails → forward_email
```

### Draft and review before sending
```
"Draft a reply to Bob's email but don't send it yet"
→ calls create_draft
"OK send that draft"
→ calls send_draft
```

### Batch organize
```
"Move all emails from newsletters@company.com to the Archive folder"
→ calls search_emails → batch_move
```

### Read a thread
```
"Show me the full conversation thread about the budget proposal"
→ calls search_emails → get_thread
```

### Download attachment
```
"Download the PDF attachment from Sarah's last email"
→ calls search_emails → get_attachments → download_attachment
```

## MCP Server Manifest

```toml
server_id = "mcp_email"
display_name = "Email"
version = "1.7.0"
domain = "collaboration"
risk_level = "medium"
writes_allowed = "gated"
transports = ["stdio"]
credentials = ["vault://email-credentials"]
governance_gates = []
environments = ["development", "staging", "production"]
```

## OAuth Setup (one-time)

```bash
# Gmail — opens browser for Google consent
mcp-email auth gmail

# Microsoft — opens browser for Microsoft consent
mcp-email auth microsoft
```

Tokens are saved to `~/.mcp-email/` and auto-refresh before expiry. No manual token management after first auth.

## Roadmap

- [ ] Email filters (create/list/delete rules)
- [ ] Multi-account support
- [ ] Webhook for incoming email (SendGrid Inbound Parse)
- [ ] Email templates (Handlebars/Tera)

## Documentation

| Document | Description |
|----------|-------------|
| [API Reference](docs/api-reference.md) | All 24 tools with parameters, types, and examples |
| [Backends](docs/backends.md) | Configuration for all 8 backends with setup guides |
| [Security](docs/security.md) | Credential handling, threat model, risk classification |
| [Architecture](docs/assets/architecture.svg) | System diagram |
| [CHANGELOG.md](CHANGELOG.md) | Version history |
| [mcp-server.toml](mcp-server.toml) | ADK-Rust Enterprise registry manifest |

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

## rmcp and MCP compatibility

This server is built with [`rmcp` 3.1.2](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.1.2) and requires Rust 1.94.1 or newer. The rmcp 3 rollout retains legacy MCP initialization compatibility and targets MCP protocol revisions `2025-11-25` and `2026-07-28`.

## MCP 2026-07-28 rollout (P0 security)

This server uses `rmcp` 3.1.2 and `adk-mcp-sdk` 0.2 with a minimum supported
Rust version of **1.94.1**. It accepts stateless MCP 2026 requests with
per-request protocol, client identity, and capability metadata while retaining
the legacy MCP 2025-11-25 initialize flow for ordinary tools.

- **Tasks:** `download_attachment`, `batch_delete`, `batch_move`, `batch_mark`
- **MRTR approvals:** `send_email`, `reply_to_email`, `move_to_folder`, `mark_read`, `create_draft`, `send_draft`, `delete_email`, `mark_unread`, `create_label`, `delete_label`, `batch_delete`, `batch_move`
- **Discovery and routing:** rmcp serves on-demand discovery and validates the
  per-request protocol envelope; HTTP deployments can route with `Mcp-Method`
  and `Mcp-Name`. The packaged binary currently uses stdio.
- **Caching:** `tools/list` returns a public `ttlMs` of 60,000 for MCP 2026;
  rmcp omits the cache fields for legacy clients.
- **Deprecated extensions:** this server does not add new Roots, Sampling, or
  dynamic client-registration dependencies.

Protected tools require `MCP_REQUEST_STATE_KEY` with at least 32 high-entropy
bytes. All replicas must share that key so sealed approval state can resume on
another instance. Approval state is bound to the client identity, tool, and
arguments and expires after two minutes. Missing identity, invalid state,
rejection, or legacy protocol use fails closed. Task records are process-local
for the current stdio runtime; use a durable task store before deploying the
server behind scale-to-zero HTTP infrastructure.

### Security hardening

Send, mutation, and destructive mailbox tools require sealed MRTR approval.
OAuth configuration files that may contain client secrets are created with
owner-only `0600` permissions on Unix, flushed before use, and covered by a
permission regression test.
