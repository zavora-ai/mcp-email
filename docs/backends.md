# Backends

mcp-email supports 5 send backends and 3 read backends. You can mix and match — e.g., send via SendGrid while reading via IMAP.

## Send Backends

### SMTP (default)

Works with any SMTP relay. Supports port 465 (implicit TLS) and port 587 (STARTTLS).

```bash
export SMTP_HOST="smtp.gmail.com"
export SMTP_PORT="587"          # or 465
export SMTP_USERNAME="you@gmail.com"
export SMTP_PASSWORD="app-password"
export SMTP_FROM="you@gmail.com"
```

**Common SMTP servers:**

| Provider | Host | Port |
|----------|------|------|
| Gmail | smtp.gmail.com | 587 or 465 |
| Outlook | smtp.office365.com | 587 |
| Yahoo | smtp.mail.yahoo.com | 465 |
| Zoho | smtp.zoho.com | 587 |
| Fastmail | smtp.fastmail.com | 465 |
| Mailgun | smtp.mailgun.org | 587 |
| Postmark | smtp.postmarkapp.com | 587 |

**Notes:**
- Gmail requires an [App Password](https://myaccount.google.com/apppasswords)
- Port 587 uses STARTTLS (upgrades plain connection to TLS)
- Port 465 uses implicit TLS (TLS from the start)

---

### SendGrid

High-volume transactional email via SendGrid's v3 API.

```bash
export SENDGRID_API_KEY="SG.xxxxxxxxxxxx"
export SENDGRID_FROM="noreply@yourdomain.com"
```

**Setup:**
1. Create account at [sendgrid.com](https://sendgrid.com)
2. Verify your sender domain
3. Create an API key with "Mail Send" permission
4. Set the env vars above

---

### AWS SES

Amazon Simple Email Service. Uses the AWS CLI for proper SigV4 signing.

```bash
export AWS_ACCESS_KEY_ID="AKIA..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_REGION="us-east-1"
export SES_FROM="noreply@yourdomain.com"
```

**Prerequisites:**
- AWS CLI installed and configured
- Sender email/domain verified in SES
- SES out of sandbox (for production sending)

**Note:** SES only activates when `SES_FROM` is explicitly set. Having AWS credentials alone won't trigger SES.

---

### Gmail API

Send via Google's Gmail API using OAuth2.

```bash
# Option 1: Manual token
export GMAIL_ACCESS_TOKEN="ya29.xxxx"

# Option 2: OAuth flow (recommended)
mcp-email auth gmail
```

**OAuth setup:**
1. Create OAuth client in [Google Cloud Console](https://console.cloud.google.com/apis/credentials)
2. Application type: Desktop app
3. Enable Gmail API
4. Add `http://localhost:8856/callback` as redirect URI
5. Run `mcp-email auth gmail`

---

### Microsoft Graph

Send via Microsoft Graph API using OAuth2.

```bash
# Option 1: Manual token
export MS_GRAPH_TOKEN="eyJ0..."

# Option 2: OAuth flow (recommended)
mcp-email auth microsoft
```

**OAuth setup:**
1. Register app in [Azure Portal](https://portal.azure.com/#blade/Microsoft_AAD_RegisteredApps)
2. Add permissions: `Mail.ReadWrite`, `Mail.Send`, `offline_access`
3. Add `http://localhost:8856/callback` as redirect URI
4. Run `mcp-email auth microsoft`

---

## Read Backends

### IMAP (default)

Works with any email provider. No OAuth needed.

```bash
export IMAP_HOST="imap.gmail.com"
export IMAP_PORT="993"
export IMAP_USERNAME="you@gmail.com"
export IMAP_PASSWORD="app-password"
```

**Common IMAP servers:**

| Provider | Host | Port |
|----------|------|------|
| Gmail | imap.gmail.com | 993 |
| Outlook | outlook.office365.com | 993 |
| Yahoo | imap.mail.yahoo.com | 993 |
| Fastmail | imap.fastmail.com | 993 |
| Zoho | imap.zoho.com | 993 |
| iCloud | imap.mail.me.com | 993 |
| ProtonMail | 127.0.0.1 (Bridge) | 1143 |

**IMAP-supported operations:**
- ✅ list_inbox
- ✅ search_emails
- ✅ mark_read / mark_unread
- ✅ star_email
- ✅ move_to_folder
- ✅ delete_email
- ✅ batch_mark
- ❌ get_thread (API-only)
- ❌ drafts (API-only)
- ❌ download_attachment (API-only)

---

### Gmail API

Full Gmail features including threads, labels, and attachment download.

```bash
mcp-email auth gmail
# or
export GMAIL_ACCESS_TOKEN="ya29.xxxx"
```

**All 24 tools supported.**

---

### Microsoft Graph

Full Outlook features including folders, categories, and attachment download.

```bash
mcp-email auth microsoft
# or
export MS_GRAPH_TOKEN="eyJ0..."
```

**All 24 tools supported.**

---

## Backend Priority

### Send priority (first match wins):
```
SMTP_HOST → SENDGRID_API_KEY+SENDGRID_FROM → SES_FROM → Gmail OAuth → Microsoft OAuth
```

### Read priority (first match wins):
```
IMAP_HOST → Gmail OAuth → Microsoft OAuth
```

---

## Combined Configurations

### Enterprise: SendGrid send + IMAP read
```bash
export SENDGRID_API_KEY="SG.xxxx"
export SENDGRID_FROM="noreply@company.com"
export IMAP_HOST="imap.gmail.com"
export IMAP_PORT="993"
export IMAP_USERNAME="support@company.com"
export IMAP_PASSWORD="app-password"
```

### AWS: SES send + IMAP read
```bash
export AWS_ACCESS_KEY_ID="AKIA..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_REGION="us-east-1"
export SES_FROM="noreply@company.com"
export IMAP_HOST="imap.gmail.com"
export IMAP_PORT="993"
export IMAP_USERNAME="inbox@company.com"
export IMAP_PASSWORD="app-password"
```

### Simple: Gmail OAuth (send + read, zero config after auth)
```bash
mcp-email auth gmail
# That's it. Both send and read use Gmail API.
```
