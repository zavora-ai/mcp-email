use adk_mcp_sdk::{HealthCheck, HealthStatus};
use crate::client::EmailClient;
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SendEmailInput {
    /// Recipient email address
    pub to: String,
    /// Email subject line
    pub subject: String,
    /// Plain text body
    pub body: String,
    /// Optional HTML body
    #[serde(default)]
    pub html: Option<String>,
    /// Optional CC recipients (comma-separated)
    #[serde(default)]
    pub cc: Option<String>,
    /// Optional BCC recipients (comma-separated)
    #[serde(default)]
    pub bcc: Option<String>,
    /// Optional file paths to attach
    #[serde(default)]
    pub attachments: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListInboxInput {
    #[serde(default = "default_20")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEmailInput {
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchEmailsInput {
    pub query: String,
    #[serde(default = "default_20")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReplyInput {
    pub message_id: String,
    pub body: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveInput {
    pub message_id: String,
    pub folder: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MessageIdInput {
    pub message_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EmptyInput {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ForwardEmailInput {
    pub message_id: String,
    /// Recipient to forward to
    pub to: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateDraftInput {
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub html: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListDraftsInput {
    #[serde(default = "default_20")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SendDraftInput {
    pub draft_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteEmailInput {
    pub message_id: String,
    /// If true, permanently delete; otherwise move to trash
    #[serde(default)]
    pub permanent: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEmailBodyInput {
    pub message_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetThreadInput {
    pub message_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DownloadAttachmentInput {
    pub message_id: String,
    pub attachment_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateLabelInput {
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteLabelInput {
    pub label_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BatchDeleteInput {
    pub message_ids: Vec<String>,
    #[serde(default)]
    pub permanent: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BatchMoveInput {
    pub message_ids: Vec<String>,
    pub folder: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BatchMarkInput {
    pub message_ids: Vec<String>,
    /// true = mark read, false = mark unread
    pub read: bool,
}

fn default_20() -> u32 { 20 }

#[derive(Clone)]
pub struct EmailServer {
    pub client: Arc<EmailClient>,
}

#[tool_router]
impl EmailServer {
    #[tool(description = "Send an email (supports plain text, HTML, CC, BCC). Uses configured backend.")]
    async fn send_email(&self, Parameters(i): Parameters<SendEmailInput>) -> String {
        match self.client.send_email(&i.to, &i.subject, &i.body, i.html.as_deref(), i.cc.as_deref(), i.bcc.as_deref(), i.attachments.as_deref()).await {
            Ok(r) => format!("Email sent via {} (id: {})", r.backend, r.message_id),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "List inbox messages")]
    async fn list_inbox(&self, Parameters(i): Parameters<ListInboxInput>) -> String {
        match self.client.list_inbox(i.limit).await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get a specific email by ID")]
    async fn get_email(&self, Parameters(i): Parameters<GetEmailInput>) -> String {
        match self.client.get_email(&i.id).await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Search emails by query")]
    async fn search_emails(&self, Parameters(i): Parameters<SearchEmailsInput>) -> String {
        match self.client.search_emails(&i.query, i.limit).await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Reply to an email")]
    async fn reply_to_email(&self, Parameters(i): Parameters<ReplyInput>) -> String {
        match self.client.reply_to_email(&i.message_id, &i.body).await {
            Ok(id) => format!("Reply sent (id: {id})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "List email labels/folders")]
    async fn list_labels(&self, Parameters(_): Parameters<EmptyInput>) -> String {
        match self.client.list_labels().await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Move an email to a folder/label")]
    async fn move_to_folder(&self, Parameters(i): Parameters<MoveInput>) -> String {
        match self.client.move_to_folder(&i.message_id, &i.folder).await {
            Ok(()) => "Moved".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Mark an email as read")]
    async fn mark_read(&self, Parameters(i): Parameters<MessageIdInput>) -> String {
        match self.client.mark_read(&i.message_id).await {
            Ok(()) => "Marked as read".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get attachments for an email")]
    async fn get_attachments(&self, Parameters(i): Parameters<MessageIdInput>) -> String {
        match self.client.get_attachments(&i.message_id).await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Forward an email to new recipients")]
    async fn forward_email(&self, Parameters(i): Parameters<ForwardEmailInput>) -> String {
        match self.client.forward_email(&i.message_id, &i.to).await {
            Ok(id) => format!("Forwarded (id: {id})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Create a draft email without sending")]
    async fn create_draft(&self, Parameters(i): Parameters<CreateDraftInput>) -> String {
        match self.client.create_draft(&i.to, &i.subject, &i.body, i.html.as_deref()).await {
            Ok(id) => format!("Draft created (id: {id})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "List saved drafts")]
    async fn list_drafts(&self, Parameters(i): Parameters<ListDraftsInput>) -> String {
        match self.client.list_drafts(i.limit).await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Send a saved draft")]
    async fn send_draft(&self, Parameters(i): Parameters<SendDraftInput>) -> String {
        match self.client.send_draft(&i.draft_id).await {
            Ok(id) => format!("Draft sent (id: {id})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Delete an email (trash or permanent)")]
    async fn delete_email(&self, Parameters(i): Parameters<DeleteEmailInput>) -> String {
        match self.client.delete_email(&i.message_id, i.permanent.unwrap_or(false)).await {
            Ok(()) => "Deleted".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Mark an email as unread")]
    async fn mark_unread(&self, Parameters(i): Parameters<MessageIdInput>) -> String {
        match self.client.mark_unread(&i.message_id).await {
            Ok(()) => "Marked as unread".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Star/flag an email")]
    async fn star_email(&self, Parameters(i): Parameters<MessageIdInput>) -> String {
        match self.client.star_email(&i.message_id).await {
            Ok(()) => "Starred".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get full email body content (HTML or text)")]
    async fn get_email_body(&self, Parameters(i): Parameters<GetEmailBodyInput>) -> String {
        match self.client.get_email_body(&i.message_id).await {
            Ok(body) => body,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get email thread/conversation")]
    async fn get_thread(&self, Parameters(i): Parameters<GetThreadInput>) -> String {
        match self.client.get_thread(&i.message_id).await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Download attachment content as base64")]
    async fn download_attachment(&self, Parameters(i): Parameters<DownloadAttachmentInput>) -> String {
        match self.client.download_attachment(&i.message_id, &i.attachment_id).await {
            Ok(data) => data,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Create a new label/folder")]
    async fn create_label(&self, Parameters(i): Parameters<CreateLabelInput>) -> String {
        match self.client.create_label(&i.name).await {
            Ok(id) => format!("Label created (id: {id})"),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Delete a label/folder")]
    async fn delete_label(&self, Parameters(i): Parameters<DeleteLabelInput>) -> String {
        match self.client.delete_label(&i.label_id).await {
            Ok(()) => "Label deleted".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Delete multiple emails at once")]
    async fn batch_delete(&self, Parameters(i): Parameters<BatchDeleteInput>) -> String {
        match self.client.batch_delete(&i.message_ids, i.permanent.unwrap_or(false)).await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Move multiple emails to a folder")]
    async fn batch_move(&self, Parameters(i): Parameters<BatchMoveInput>) -> String {
        match self.client.batch_move(&i.message_ids, &i.folder).await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Mark multiple emails as read or unread")]
    async fn batch_mark(&self, Parameters(i): Parameters<BatchMarkInput>) -> String {
        match self.client.batch_mark(&i.message_ids, i.read).await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {e}"),
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for EmailServer {
    async fn check_health(&self) -> HealthStatus {
        HealthStatus { healthy: true, message: Some("operational".into()), latency_ms: Some(1) }
    }
}

adk_mcp_sdk::mcp_2026_server! {
    server: EmailServer,
    task_tools: ["download_attachment", "batch_delete", "batch_move", "batch_mark"],
    approval_tools: ["send_email", "reply_to_email", "move_to_folder", "mark_read", "create_draft", "send_draft", "delete_email", "mark_unread", "create_label", "delete_label", "batch_delete", "batch_move"],
    cache_ttl_ms: 60_000,
}
