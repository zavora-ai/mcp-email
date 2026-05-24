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
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListInboxInput {
    /// Max messages to return (default 20)
    #[serde(default = "default_20")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetEmailInput {
    /// Message ID
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchEmailsInput {
    /// Search query (Gmail syntax or Microsoft $search)
    pub query: String,
    /// Max results (default 20)
    #[serde(default = "default_20")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReplyInput {
    /// Message ID to reply to
    pub message_id: String,
    /// Reply body text
    pub body: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveInput {
    /// Message ID
    pub message_id: String,
    /// Target folder/label ID
    pub folder: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MarkReadInput {
    /// Message ID
    pub message_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAttachmentsInput {
    /// Message ID
    pub message_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EmptyInput {}

fn default_20() -> u32 { 20 }

#[derive(Clone)]
pub struct EmailServer {
    pub client: Arc<EmailClient>,
}

#[tool_router(server_handler)]
impl EmailServer {
    #[tool(description = "Send an email (supports plain text and HTML). Uses configured backend: SMTP, SendGrid, AWS SES, Gmail, or Microsoft Graph.")]
    async fn send_email(&self, Parameters(i): Parameters<SendEmailInput>) -> String {
        match self.client.send_email(&i.to, &i.subject, &i.body, i.html.as_deref()).await {
            Ok(r) => format!("Email sent via {} (id: {})", r.backend, r.message_id),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "List inbox messages (requires Gmail or Microsoft Graph read backend)")]
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
    async fn mark_read(&self, Parameters(i): Parameters<MarkReadInput>) -> String {
        match self.client.mark_read(&i.message_id).await {
            Ok(()) => "Marked as read".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get attachments for an email")]
    async fn get_attachments(&self, Parameters(i): Parameters<GetAttachmentsInput>) -> String {
        match self.client.get_attachments(&i.message_id).await {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
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
