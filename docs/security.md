# Security Model

## Credential Handling

- **No secrets in LLM context** — tokens and passwords stay in env vars or `~/.mcp-email/`, never returned in tool responses
- **OAuth tokens** stored at `~/.mcp-email/{provider}_token.json` with `600` permissions (owner-only)
- **Auto-refresh** — tokens refresh 5 minutes before expiry, no manual intervention
- **No credential logging** — tracing never logs token values, only backend names

## File Permissions

```
~/.mcp-email/
├── gmail_token.json      (600 — owner read/write only)
├── gmail_oauth.json      (644 — client_id is not secret)
├── microsoft_token.json  (600)
└── microsoft_oauth.json  (644)
```

## Attack Surface

| Vector | Mitigation |
|--------|-----------|
| Token theft from disk | File permissions 600, stored in user home |
| Token in transit | All API calls over HTTPS/TLS |
| SMTP credential sniffing | TLS required (port 465 or STARTTLS on 587) |
| IMAP credential sniffing | TLS required (port 993) |
| Attachment path traversal | Files read from absolute paths only, no directory listing |
| LLM prompt injection via email | Tool returns metadata by default, full body only on explicit request |

## Risk Classification

| Tool | Risk Class | Approval Required |
|------|-----------|:-:|
| list_inbox, get_email, search_emails | Read-only | No |
| send_email, reply, forward | External write | Yes (production) |
| delete_email, batch_delete | Destructive | Yes |
| create_label, delete_label | Internal write | No |
| mark_read, move_to_folder | Internal write | No |

## Recommendations

1. **Use App Passwords** for SMTP/IMAP (not your main password)
2. **Scope OAuth minimally** — only request `https://mail.google.com/` for Gmail
3. **Rotate tokens** — revoke and re-auth periodically
4. **Use SES/SendGrid for production** — better deliverability and audit trails than SMTP
5. **Set `RUST_LOG=warn`** in production to minimize log verbosity
