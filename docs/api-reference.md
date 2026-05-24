# API Reference

## Tools (24)

---

### send_email

Send an email with optional HTML, CC/BCC, and file attachments.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `to` | string | ✅ | Recipient email address |
| `subject` | string | ✅ | Subject line |
| `body` | string | ✅ | Plain text body |
| `html` | string | ❌ | HTML body (sent as multipart/alternative) |
| `cc` | string | ❌ | CC recipients (comma-separated) |
| `bcc` | string | ❌ | BCC recipients (comma-separated) |
| `attachments` | string[] | ❌ | Absolute file paths to attach |

**Example:**
```json
{
  "to": "recipient@example.com",
  "subject": "Q3 Report",
  "body": "Please find the report attached.",
  "attachments": ["/path/to/report.pdf", "/path/to/data.xlsx"]
}
```

**Returns:** `"Email sent via {backend} (id: {message_id})"`

---

### list_inbox

List recent inbox messages.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `limit` | u32 | ❌ | Max messages (default 20) |

**Returns:** Array of `EmailMessage` objects.

---

### get_email

Get email metadata by ID.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `id` | string | ✅ | Message ID |

**Returns:** `EmailMessage` with subject, from, to, date, is_read.

---

### get_email_body

Get the full body content of an email (HTML or plain text).

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |

**Returns:** Full email body as string.

---

### search_emails

Search emails by query.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `query` | string | ✅ | Search query (Gmail syntax or IMAP SEARCH) |
| `limit` | u32 | ❌ | Max results (default 20) |

**Gmail query examples:** `from:alice@example.com`, `has:attachment`, `after:2024/01/01`, `subject:"meeting"`

**Returns:** Array of `EmailMessage` objects.

---

### reply_to_email

Reply to an email (preserves threading).

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID to reply to |
| `body` | string | ✅ | Reply body text |

---

### forward_email

Forward an email to new recipients.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID to forward |
| `to` | string | ✅ | Recipient email address |

---

### create_draft

Save a draft without sending.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `to` | string | ✅ | Recipient |
| `subject` | string | ✅ | Subject |
| `body` | string | ✅ | Body text |
| `html` | string | ❌ | HTML body |

**Returns:** Draft ID.

---

### list_drafts

List saved drafts.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `limit` | u32 | ❌ | Max drafts (default 20) |

---

### send_draft

Send a previously saved draft.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `draft_id` | string | ✅ | Draft ID |

---

### list_labels

List all email labels/folders.

No parameters required.

---

### get_attachments

Get attachment metadata for an email.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |

**Returns:** Array of `Attachment` objects with id, filename, mime_type, size.

---

### download_attachment

Download attachment content as base64.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |
| `attachment_id` | string | ✅ | Attachment ID (from get_attachments) |

**Returns:** Base64-encoded file content.

---

### get_thread

Get an entire email thread/conversation.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Any message ID in the thread |

**Returns:** Array of `EmailMessage` objects in the thread.

---

### move_to_folder

Move an email to a folder/label.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |
| `folder` | string | ✅ | Target folder/label ID |

---

### mark_read

Mark an email as read.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |

---

### mark_unread

Mark an email as unread.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |

---

### star_email

Star/flag a message.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |

---

### delete_email

Trash or permanently delete an email.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_id` | string | ✅ | Message ID |
| `permanent` | bool | ❌ | Permanent delete (default: trash) |

---

### create_label

Create a new label/folder.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `name` | string | ✅ | Label name |

---

### delete_label

Delete a label/folder.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `label_id` | string | ✅ | Label ID |

---

### batch_delete

Delete multiple emails at once.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_ids` | string[] | ✅ | Array of message IDs |
| `permanent` | bool | ❌ | Permanent delete (default: trash) |

---

### batch_move

Move multiple emails to a folder.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_ids` | string[] | ✅ | Array of message IDs |
| `folder` | string | ✅ | Target folder/label ID |

---

### batch_mark

Mark multiple emails read/unread.

| Field | Type | Required | Description |
|-------|------|:---:|-------------|
| `message_ids` | string[] | ✅ | Array of message IDs |
| `read` | bool | ✅ | true = mark read, false = mark unread |

---

## Data Types

### EmailMessage

```json
{
  "id": "19e5a2cdc0f9a2f5",
  "subject": "Q3 Report",
  "from": "alice@example.com",
  "to": "bob@example.com",
  "snippet": "Please find the report...",
  "date": "Sun, 24 May 2026 16:33:05 +0000",
  "is_read": true
}
```

### Attachment

```json
{
  "id": "ANGjdJ9fkTs...",
  "filename": "report.pdf",
  "mime_type": "application/pdf",
  "size": 245760
}
```

### SendResult

```json
{
  "message_id": "19e5a2cdc0f9a2f5",
  "backend": "gmail"
}
```
