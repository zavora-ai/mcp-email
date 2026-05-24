mod client;
mod oauth;
mod server;

use client::{EmailClient, ReadBackend, SendBackend};
use rmcp::{ServiceExt, transport::stdio};
use server::EmailServer;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install rustls crypto provider");

    // Handle `mcp-email auth gmail` or `mcp-email auth microsoft`
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "auth" {
        let provider = &args[2];
        let client_id = std::env::var(format!("{}_CLIENT_ID", provider.to_uppercase()))
            .or_else(|_| {
                println!("Enter OAuth Client ID:");
                let mut s = String::new();
                std::io::stdin().read_line(&mut s).ok();
                Ok::<_, std::env::VarError>(s.trim().to_string())
            })?;
        let client_secret = std::env::var(format!("{}_CLIENT_SECRET", provider.to_uppercase())).ok()
            .or_else(|| {
                println!("Enter Client Secret (or press Enter for PKCE-only):");
                let mut s = String::new();
                std::io::stdin().read_line(&mut s).ok();
                let s = s.trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            });
        oauth::run_auth_flow(provider, &client_id, client_secret.as_deref()).await?;
        return Ok(());
    }

    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?)).init();

    // Detect send backend (priority: SMTP > SendGrid > SES > Gmail > Microsoft)
    let send_backend = if let (Ok(host), Ok(user), Ok(pass)) = (
        std::env::var("SMTP_HOST"), std::env::var("SMTP_USERNAME"), std::env::var("SMTP_PASSWORD")
    ) {
        let port = std::env::var("SMTP_PORT").unwrap_or_else(|_| "587".into()).parse().unwrap_or(587);
        let from = std::env::var("SMTP_FROM").or_else(|_| std::env::var("EMAIL_FROM")).unwrap_or_else(|_| user.clone());
        tracing::info!(backend = "smtp", host = %host, port, "Send backend configured");
        SendBackend::Smtp { host, port, username: user, password: pass, from }
    } else if let Ok(api_key) = std::env::var("SENDGRID_API_KEY") {
        let from = std::env::var("SENDGRID_FROM").or_else(|_| std::env::var("EMAIL_FROM")).expect("SENDGRID_FROM or EMAIL_FROM required");
        tracing::info!(backend = "sendgrid", "Send backend configured");
        SendBackend::SendGrid { api_key, from }
    } else if let (Ok(access_key), Ok(secret_key)) = (std::env::var("AWS_ACCESS_KEY_ID"), std::env::var("AWS_SECRET_ACCESS_KEY")) {
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into());
        let from = std::env::var("SES_FROM").or_else(|_| std::env::var("EMAIL_FROM")).expect("SES_FROM or EMAIL_FROM required");
        tracing::info!(backend = "ses", region = %region, "Send backend configured");
        SendBackend::Ses { region, access_key, secret_key, from }
    } else if let Ok(token) = resolve_gmail_token().await {
        tracing::info!(backend = "gmail", "Send backend configured (OAuth)");
        SendBackend::Gmail { token }
    } else if let Ok(token) = resolve_microsoft_token().await {
        tracing::info!(backend = "microsoft", "Send backend configured (OAuth)");
        SendBackend::Microsoft { token }
    } else {
        anyhow::bail!("No send backend. Set SMTP_HOST, SENDGRID_API_KEY, AWS_ACCESS_KEY_ID, or run: mcp-email auth gmail");
    };

    // Detect read backend (priority: IMAP > Gmail > Microsoft)
    let read_backend = if let (Ok(host), Ok(user), Ok(pass)) = (
        std::env::var("IMAP_HOST"), std::env::var("IMAP_USERNAME"), std::env::var("IMAP_PASSWORD")
    ) {
        let port = std::env::var("IMAP_PORT").unwrap_or_else(|_| "993".into()).parse().unwrap_or(993);
        tracing::info!(backend = "imap", host = %host, "Read backend configured");
        Some(ReadBackend::Imap { host, port, username: user, password: pass })
    } else if let Ok(token) = resolve_gmail_token().await {
        tracing::info!(backend = "gmail", "Read backend configured (OAuth)");
        Some(ReadBackend::Gmail { token })
    } else if let Ok(token) = resolve_microsoft_token().await {
        tracing::info!(backend = "microsoft", "Read backend configured (OAuth)");
        Some(ReadBackend::Microsoft { token })
    } else {
        None
    };

    let client = Arc::new(EmailClient::new(send_backend, read_backend));
    tracing::info!(send = client.backend_name(), "mcp-email starting on stdio");
    let server = EmailServer { client };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// Try to get Gmail token from env var or OAuth store
async fn resolve_gmail_token() -> anyhow::Result<String> {
    if let Ok(token) = std::env::var("GMAIL_ACCESS_TOKEN") {
        return Ok(token);
    }
    oauth::get_valid_token("gmail").await
}

/// Try to get Microsoft token from env var or OAuth store
async fn resolve_microsoft_token() -> anyhow::Result<String> {
    if let Ok(token) = std::env::var("MS_GRAPH_TOKEN") {
        return Ok(token);
    }
    oauth::get_valid_token("microsoft").await
}
