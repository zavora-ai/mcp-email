mod client;
mod server;

use client::{EmailClient, EmailProvider};
use rmcp::{ServiceExt, transport::stdio};
use server::EmailServer;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?)).init();
    let provider = if let Ok(token) = std::env::var("GMAIL_ACCESS_TOKEN") {
        EmailProvider::Gmail { token }
    } else if let Ok(token) = std::env::var("MS_GRAPH_TOKEN") {
        EmailProvider::Microsoft { token }
    } else {
        panic!("GMAIL_ACCESS_TOKEN or MS_GRAPH_TOKEN required");
    };
    let client = Arc::new(EmailClient::new(provider));
    let server = EmailServer { client };
    tracing::info!("mcp-email starting on stdio");
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
