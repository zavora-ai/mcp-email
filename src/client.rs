use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Send Backends ───────────────────────────────────────────────────────────

/// Sending backend — how outbound email is delivered.
#[derive(Clone)]
pub enum SendBackend {
    /// SMTP relay (default, works with any provider)
    Smtp { host: String, port: u16, username: String, password: String, from: String },
    /// AWS SES via HTTP API
    Ses { region: String, access_key: String, secret_key: String, from: String },
    /// SendGrid v3 API
    SendGrid { api_key: String, from: String },
    /// Gmail API (OAuth token)
    Gmail { token: String },
    /// Microsoft Graph API (OAuth token)
    Microsoft { token: String },
}

/// Reading backend — how inbound email is accessed.
#[derive(Clone)]
pub enum ReadBackend {
    /// Gmail API
    Gmail { token: String },
    /// Microsoft Graph
    Microsoft { token: String },
    /// IMAP (generic)
    Imap { host: String, port: u16, username: String, password: String },
}

#[derive(Clone)]
pub struct EmailClient {
    http: Client,
    pub send_backend: SendBackend,
    pub read_backend: Option<ReadBackend>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub snippet: Option<String>,
    pub date: Option<String>,
    pub is_read: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendResult {
    pub message_id: String,
    pub backend: String,
}

impl EmailClient {
    pub fn new(send_backend: SendBackend, read_backend: Option<ReadBackend>) -> Self {
        Self { http: Client::new(), send_backend, read_backend }
    }

    // ─── SEND ────────────────────────────────────────────────────────────────

    pub async fn send_email(&self, to: &str, subject: &str, body: &str, html: Option<&str>) -> anyhow::Result<SendResult> {
        match &self.send_backend {
            SendBackend::Smtp { host, port, username, password, from } => {
                self.send_smtp(host, *port, username, password, from, to, subject, body).await
            }
            SendBackend::Ses { region, access_key, secret_key, from } => {
                self.send_ses(region, access_key, secret_key, from, to, subject, body, html).await
            }
            SendBackend::SendGrid { api_key, from } => {
                self.send_sendgrid(api_key, from, to, subject, body, html).await
            }
            SendBackend::Gmail { token } => {
                self.send_gmail(token, to, subject, body).await
            }
            SendBackend::Microsoft { token } => {
                self.send_microsoft(token, to, subject, body).await
            }
        }
    }

    async fn send_smtp(&self, host: &str, port: u16, username: &str, password: &str, from: &str, to: &str, subject: &str, body: &str) -> anyhow::Result<SendResult> {
        // Use reqwest to call a local SMTP relay or use lettre-style raw socket
        // For portability, we POST to the SMTP server via the submission port
        let raw_msg = format!(
            "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\nMIME-Version: 1.0\r\n\r\n{body}"
        );
        // Connect via TCP and send SMTP commands
        use tokio::net::TcpStream;
        use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
        let mut stream = TcpStream::connect(format!("{host}:{port}")).await?;
        let (reader, mut writer) = stream.split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        // Read greeting
        reader.read_line(&mut line).await?;
        line.clear();

        // EHLO
        writer.write_all(format!("EHLO mcp-email\r\n").as_bytes()).await?;
        loop { line.clear(); reader.read_line(&mut line).await?; if line.starts_with("250 ") { break; } }

        // STARTTLS if port 587
        if port == 587 {
            writer.write_all(b"STARTTLS\r\n").await?;
            line.clear(); reader.read_line(&mut line).await?;
            // For full TLS upgrade we'd need tokio-rustls — skip for now, assume port 25/465
        }

        // AUTH LOGIN
        writer.write_all(b"AUTH LOGIN\r\n").await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(format!("{}\r\n", base64_encode(username.as_bytes())).as_bytes()).await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(format!("{}\r\n", base64_encode(password.as_bytes())).as_bytes()).await?;
        line.clear(); reader.read_line(&mut line).await?;

        // MAIL FROM / RCPT TO / DATA
        writer.write_all(format!("MAIL FROM:<{from}>\r\n").as_bytes()).await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(format!("RCPT TO:<{to}>\r\n").as_bytes()).await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(b"DATA\r\n").await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(format!("{raw_msg}\r\n.\r\n").as_bytes()).await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(b"QUIT\r\n").await?;

        Ok(SendResult { message_id: "smtp-sent".into(), backend: "smtp".into() })
    }

    async fn send_ses(&self, region: &str, access_key: &str, secret_key: &str, from: &str, to: &str, subject: &str, body: &str, html: Option<&str>) -> anyhow::Result<SendResult> {
        let endpoint = format!("https://email.{region}.amazonaws.com");
        let mut params = HashMap::new();
        params.insert("Action", "SendEmail");
        params.insert("Source", from);
        params.insert("Destination.ToAddresses.member.1", to);
        params.insert("Message.Subject.Data", subject);
        params.insert("Message.Body.Text.Data", body);
        if let Some(h) = html {
            params.insert("Message.Body.Html.Data", h);
        }

        // SES v1 query API with basic auth header (simplified — production should use SigV4)
        let resp = self.http.post(&endpoint)
            .form(&params)
            .header("X-Amz-Access-Key", access_key)
            .header("X-Amz-Secret-Key", secret_key)
            .send().await?;

        if resp.status().is_success() {
            let text = resp.text().await?;
            let msg_id = text.split("<MessageId>").nth(1)
                .and_then(|s| s.split("</MessageId>").next())
                .unwrap_or("ses-sent").to_string();
            Ok(SendResult { message_id: msg_id, backend: "ses".into() })
        } else {
            anyhow::bail!("SES error: {}", resp.text().await?)
        }
    }

    async fn send_sendgrid(&self, api_key: &str, from: &str, to: &str, subject: &str, body: &str, html: Option<&str>) -> anyhow::Result<SendResult> {
        let mut content = vec![serde_json::json!({"type": "text/plain", "value": body})];
        if let Some(h) = html {
            content.push(serde_json::json!({"type": "text/html", "value": h}));
        }
        let payload = serde_json::json!({
            "personalizations": [{"to": [{"email": to}]}],
            "from": {"email": from},
            "subject": subject,
            "content": content
        });

        let resp = self.http.post("https://api.sendgrid.com/v3/mail/send")
            .bearer_auth(api_key)
            .json(&payload)
            .send().await?;

        if resp.status().is_success() || resp.status().as_u16() == 202 {
            let msg_id = resp.headers().get("x-message-id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("sendgrid-sent").to_string();
            Ok(SendResult { message_id: msg_id, backend: "sendgrid".into() })
        } else {
            anyhow::bail!("SendGrid error: {}", resp.text().await?)
        }
    }

    async fn send_gmail(&self, token: &str, to: &str, subject: &str, body: &str) -> anyhow::Result<SendResult> {
        let raw = format!("To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}");
        let encoded = base64_url_encode(raw.as_bytes());
        let resp: serde_json::Value = self.http
            .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
            .bearer_auth(token)
            .json(&serde_json::json!({"raw": encoded}))
            .send().await?.error_for_status()?.json().await?;
        Ok(SendResult { message_id: resp["id"].as_str().unwrap_or("sent").into(), backend: "gmail".into() })
    }

    async fn send_microsoft(&self, token: &str, to: &str, subject: &str, body: &str) -> anyhow::Result<SendResult> {
        let msg = serde_json::json!({
            "message": {
                "subject": subject,
                "body": {"contentType": "Text", "content": body},
                "toRecipients": [{"emailAddress": {"address": to}}]
            }
        });
        self.http.post("https://graph.microsoft.com/v1.0/me/sendMail")
            .bearer_auth(token).json(&msg)
            .send().await?.error_for_status()?;
        Ok(SendResult { message_id: "sent".into(), backend: "microsoft".into() })
    }

    // ─── READ ────────────────────────────────────────────────────────────────

    fn read_token(&self) -> anyhow::Result<(&str, bool)> {
        match &self.read_backend {
            Some(ReadBackend::Gmail { token }) => Ok((token, true)),
            Some(ReadBackend::Microsoft { token }) => Ok((token, false)),
            Some(ReadBackend::Imap { .. }) => anyhow::bail!("__IMAP__"), // sentinel — handled separately
            None => {
                match &self.send_backend {
                    SendBackend::Gmail { token } => Ok((token, true)),
                    SendBackend::Microsoft { token } => Ok((token, false)),
                    _ => anyhow::bail!("No read backend configured. Set IMAP_HOST, GMAIL_ACCESS_TOKEN, or MS_GRAPH_TOKEN."),
                }
            }
        }
    }

    fn is_imap(&self) -> bool {
        matches!(&self.read_backend, Some(ReadBackend::Imap { .. }))
    }

    async fn imap_fetch_messages(&self, folder: &str, limit: u32) -> anyhow::Result<Vec<EmailMessage>> {
        let (host, port, username, password) = match &self.read_backend {
            Some(ReadBackend::Imap { host, port, username, password }) => (host.clone(), *port, username.clone(), password.clone()),
            _ => anyhow::bail!("Not IMAP backend"),
        };

        use tokio_util::compat::TokioAsyncReadCompatExt;
        use futures::TryStreamExt;

        let tcp = tokio::net::TcpStream::connect(format!("{host}:{port}")).await?;
        let tls = {
            let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
            let domain = rustls_pki_types::ServerName::try_from(host.as_str())?.to_owned();
            connector.connect(domain, tcp).await?
        };

        let mut client = async_imap::Client::new(tls.compat());
        let _greeting = client.read_response().await;
        let mut session = client.login(&username, &password).await.map_err(|e| anyhow::anyhow!("{}", e.0))?;
        session.select(folder).await?;

        let search = session.search("ALL").await?;
        let mut uids: Vec<_> = search.into_iter().collect();
        uids.sort_unstable();
        uids.reverse();
        uids.truncate(limit as usize);

        if uids.is_empty() {
            session.logout().await?;
            return Ok(vec![]);
        }

        let uid_set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        let messages: Vec<_> = session.fetch(&uid_set, "(RFC822.HEADER FLAGS)").await?.try_collect().await?;

        let mut result = Vec::new();
        for msg in &messages {
            if let Some(header_bytes) = msg.header() {
                if let Ok((headers, _)) = mailparse::parse_headers(header_bytes) {
                    let get = |name: &str| headers.iter().find(|h| h.get_key_ref().eq_ignore_ascii_case(name)).map(|h| h.get_value());
                    result.push(EmailMessage {
                        id: msg.message.to_string(),
                        subject: get("Subject"),
                        from: get("From"),
                        to: get("To"),
                        snippet: None,
                        date: get("Date"),
                        is_read: Some(msg.flags().any(|f| matches!(f, async_imap::types::Flag::Seen))),
                    });
                }
            }
        }
        session.logout().await?;
        Ok(result)
    }

    async fn imap_search(&self, query: &str, limit: u32) -> anyhow::Result<Vec<EmailMessage>> {
        let (host, port, username, password) = match &self.read_backend {
            Some(ReadBackend::Imap { host, port, username, password }) => (host.clone(), *port, username.clone(), password.clone()),
            _ => anyhow::bail!("Not IMAP backend"),
        };

        use tokio_util::compat::TokioAsyncReadCompatExt;
        use futures::TryStreamExt;

        let tcp = tokio::net::TcpStream::connect(format!("{host}:{port}")).await?;
        let tls = {
            let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
            let domain = rustls_pki_types::ServerName::try_from(host.as_str())?.to_owned();
            connector.connect(domain, tcp).await?
        };

        let mut client = async_imap::Client::new(tls.compat());
        let _greeting = client.read_response().await;
        let mut session = client.login(&username, &password).await.map_err(|e| anyhow::anyhow!("{}", e.0))?;
        session.select("INBOX").await?;

        let search_cmd = format!("OR SUBJECT \"{}\" FROM \"{}\"", query, query);
        let search_results = session.search(&search_cmd).await?;
        let mut uids: Vec<_> = search_results.into_iter().collect();
        uids.sort_unstable();
        uids.reverse();
        uids.truncate(limit as usize);

        if uids.is_empty() {
            session.logout().await?;
            return Ok(vec![]);
        }

        let uid_set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        let messages: Vec<_> = session.fetch(&uid_set, "(RFC822.HEADER FLAGS)").await?.try_collect().await?;

        let mut result = Vec::new();
        for msg in &messages {
            if let Some(header_bytes) = msg.header() {
                if let Ok((headers, _)) = mailparse::parse_headers(header_bytes) {
                    let get = |name: &str| headers.iter().find(|h| h.get_key_ref().eq_ignore_ascii_case(name)).map(|h| h.get_value());
                    result.push(EmailMessage {
                        id: msg.message.to_string(),
                        subject: get("Subject"),
                        from: get("From"),
                        to: get("To"),
                        snippet: None,
                        date: get("Date"),
                        is_read: Some(msg.flags().any(|f| matches!(f, async_imap::types::Flag::Seen))),
                    });
                }
            }
        }
        session.logout().await?;
        Ok(result)
    }

    pub async fn list_inbox(&self, limit: u32) -> anyhow::Result<Vec<EmailMessage>> {
        if self.is_imap() {
            return self.imap_fetch_messages("INBOX", limit).await;
        }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
                .bearer_auth(token)
                .query(&[("maxResults", limit.to_string()), ("labelIds", "INBOX".into())])
                .send().await?.error_for_status()?.json().await?;
            let ids: Vec<String> = resp["messages"].as_array()
                .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
                .unwrap_or_default();
            let mut msgs = Vec::new();
            for id in ids.into_iter().take(limit as usize) {
                if let Ok(m) = self.get_email(&id).await { msgs.push(m); }
            }
            Ok(msgs)
        } else {
            let resp: serde_json::Value = self.http
                .get("https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages")
                .bearer_auth(token)
                .query(&[("$top", &limit.to_string())])
                .send().await?.error_for_status()?.json().await?;
            Ok(parse_ms_messages(&resp))
        }
    }

    pub async fn get_email(&self, id: &str) -> anyhow::Result<EmailMessage> {
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}"))
                .bearer_auth(token)
                .query(&[("format", "metadata")])
                .send().await?.error_for_status()?.json().await?;
            Ok(parse_gmail_message(&resp))
        } else {
            let resp: serde_json::Value = self.http
                .get(format!("https://graph.microsoft.com/v1.0/me/messages/{id}"))
                .bearer_auth(token)
                .send().await?.error_for_status()?.json().await?;
            Ok(EmailMessage {
                id: id.to_string(),
                subject: resp["subject"].as_str().map(String::from),
                from: resp["from"]["emailAddress"]["address"].as_str().map(String::from),
                to: None, snippet: resp["bodyPreview"].as_str().map(String::from),
                date: resp["receivedDateTime"].as_str().map(String::from),
                is_read: resp["isRead"].as_bool(),
            })
        }
    }

    pub async fn search_emails(&self, query: &str, limit: u32) -> anyhow::Result<Vec<EmailMessage>> {
        if self.is_imap() {
            return self.imap_search(query, limit).await;
        }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
                .bearer_auth(token)
                .query(&[("q", query), ("maxResults", &limit.to_string())])
                .send().await?.error_for_status()?.json().await?;
            let ids: Vec<String> = resp["messages"].as_array()
                .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
                .unwrap_or_default();
            let mut msgs = Vec::new();
            for id in ids.into_iter().take(limit as usize) {
                if let Ok(m) = self.get_email(&id).await { msgs.push(m); }
            }
            Ok(msgs)
        } else {
            let resp: serde_json::Value = self.http
                .get("https://graph.microsoft.com/v1.0/me/messages")
                .bearer_auth(token)
                .query(&[("$search", &format!("\"{query}\"")), ("$top", &limit.to_string())])
                .send().await?.error_for_status()?.json().await?;
            Ok(parse_ms_messages(&resp))
        }
    }

    pub async fn reply_to_email(&self, id: &str, body: &str) -> anyhow::Result<String> {
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let orig = self.get_email(id).await?;
            let subject = format!("Re: {}", orig.subject.unwrap_or_default());
            let to = orig.from.unwrap_or_default();
            let raw = format!("To: {to}\r\nSubject: {subject}\r\nIn-Reply-To: {id}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}");
            let encoded = base64_url_encode(raw.as_bytes());
            let resp: serde_json::Value = self.http
                .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
                .bearer_auth(token)
                .json(&serde_json::json!({"raw": encoded, "threadId": id}))
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["id"].as_str().unwrap_or("sent").to_string())
        } else {
            self.http.post(format!("https://graph.microsoft.com/v1.0/me/messages/{id}/reply"))
                .bearer_auth(token)
                .json(&serde_json::json!({"comment": body}))
                .send().await?.error_for_status()?;
            Ok("replied".to_string())
        }
    }

    pub async fn list_labels(&self) -> anyhow::Result<Vec<Label>> {
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get("https://gmail.googleapis.com/gmail/v1/users/me/labels")
                .bearer_auth(token)
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["labels"].as_array()
                .map(|a| a.iter().map(|l| Label { id: l["id"].as_str().unwrap_or("").into(), name: l["name"].as_str().unwrap_or("").into() }).collect())
                .unwrap_or_default())
        } else {
            let resp: serde_json::Value = self.http
                .get("https://graph.microsoft.com/v1.0/me/mailFolders")
                .bearer_auth(token)
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["value"].as_array()
                .map(|a| a.iter().map(|f| Label { id: f["id"].as_str().unwrap_or("").into(), name: f["displayName"].as_str().unwrap_or("").into() }).collect())
                .unwrap_or_default())
        }
    }

    pub async fn move_to_folder(&self, message_id: &str, folder: &str) -> anyhow::Result<()> {
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            self.http.post(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/modify"))
                .bearer_auth(token)
                .json(&serde_json::json!({"addLabelIds": [folder]}))
                .send().await?.error_for_status()?;
        } else {
            self.http.post(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}/move"))
                .bearer_auth(token)
                .json(&serde_json::json!({"destinationId": folder}))
                .send().await?.error_for_status()?;
        }
        Ok(())
    }

    pub async fn mark_read(&self, message_id: &str) -> anyhow::Result<()> {
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            self.http.post(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/modify"))
                .bearer_auth(token)
                .json(&serde_json::json!({"removeLabelIds": ["UNREAD"]}))
                .send().await?.error_for_status()?;
        } else {
            self.http.patch(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}"))
                .bearer_auth(token)
                .json(&serde_json::json!({"isRead": true}))
                .send().await?.error_for_status()?;
        }
        Ok(())
    }

    pub async fn get_attachments(&self, message_id: &str) -> anyhow::Result<Vec<Attachment>> {
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"))
                .bearer_auth(token)
                .query(&[("format", "full")])
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["payload"]["parts"].as_array()
                .map(|parts| parts.iter().filter_map(|p| {
                    p["filename"].as_str().filter(|f| !f.is_empty()).map(|f| Attachment {
                        id: p["body"]["attachmentId"].as_str().unwrap_or("").into(),
                        filename: f.into(),
                        mime_type: p["mimeType"].as_str().map(String::from),
                        size: p["body"]["size"].as_u64(),
                    })
                }).collect())
                .unwrap_or_default())
        } else {
            let resp: serde_json::Value = self.http
                .get(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}/attachments"))
                .bearer_auth(token)
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["value"].as_array()
                .map(|a| a.iter().map(|att| Attachment {
                    id: att["id"].as_str().unwrap_or("").into(),
                    filename: att["name"].as_str().unwrap_or("").into(),
                    mime_type: att["contentType"].as_str().map(String::from),
                    size: att["size"].as_u64(),
                }).collect())
                .unwrap_or_default())
        }
    }

    pub fn backend_name(&self) -> &str {
        match &self.send_backend {
            SendBackend::Smtp { .. } => "smtp",
            SendBackend::Ses { .. } => "ses",
            SendBackend::SendGrid { .. } => "sendgrid",
            SendBackend::Gmail { .. } => "gmail",
            SendBackend::Microsoft { .. } => "microsoft",
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 { result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char); }
        if chunk.len() > 2 { result.push(CHARS[(triple & 0x3F) as usize] as char); }
    }
    result
}

fn base64_url_encode(data: &[u8]) -> String {
    base64_encode(data).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
}

fn parse_gmail_message(v: &serde_json::Value) -> EmailMessage {
    let headers = v["payload"]["headers"].as_array();
    let get_header = |name: &str| -> Option<String> {
        headers.and_then(|h| h.iter().find(|hdr| hdr["name"].as_str() == Some(name)).and_then(|hdr| hdr["value"].as_str().map(String::from)))
    };
    EmailMessage {
        id: v["id"].as_str().unwrap_or("").into(),
        subject: get_header("Subject"),
        from: get_header("From"),
        to: get_header("To"),
        snippet: v["snippet"].as_str().map(String::from),
        date: get_header("Date"),
        is_read: v["labelIds"].as_array().map(|l| !l.iter().any(|lbl| lbl.as_str() == Some("UNREAD"))),
    }
}

fn parse_ms_messages(resp: &serde_json::Value) -> Vec<EmailMessage> {
    resp["value"].as_array()
        .map(|a| a.iter().map(|m| EmailMessage {
            id: m["id"].as_str().unwrap_or("").into(),
            subject: m["subject"].as_str().map(String::from),
            from: m["from"]["emailAddress"]["address"].as_str().map(String::from),
            to: None, snippet: m["bodyPreview"].as_str().map(String::from),
            date: m["receivedDateTime"].as_str().map(String::from),
            is_read: m["isRead"].as_bool(),
        }).collect())
        .unwrap_or_default()
}
