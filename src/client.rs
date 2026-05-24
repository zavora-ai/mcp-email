use reqwest::Client;
use serde::{Deserialize, Serialize};

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

    pub async fn send_email(&self, to: &str, subject: &str, body: &str, html: Option<&str>, cc: Option<&str>, bcc: Option<&str>, attachments: Option<&[String]>) -> anyhow::Result<SendResult> {
        match &self.send_backend {
            SendBackend::Smtp { host, port, username, password, from } => {
                let raw = self.build_mime(from, to, cc, bcc, subject, body, html, attachments)?;
                self.send_smtp_raw(host, *port, username, password, from, to, &raw).await
            }
            SendBackend::Ses { region, access_key, secret_key, from } => {
                self.send_ses(region, access_key, secret_key, from, to, subject, body, html).await
            }
            SendBackend::SendGrid { api_key, from } => {
                self.send_sendgrid(api_key, from, to, subject, body, html).await
            }
            SendBackend::Gmail { token } => {
                let raw = self.build_mime("me", to, cc, bcc, subject, body, html, attachments)?;
                self.send_gmail_raw(token, &raw).await
            }
            SendBackend::Microsoft { token } => {
                self.send_microsoft(token, to, subject, body).await
            }
        }
    }

    fn build_mime(&self, from: &str, to: &str, cc: Option<&str>, bcc: Option<&str>, subject: &str, body: &str, html: Option<&str>, attachments: Option<&[String]>) -> anyhow::Result<String> {
        use mail_builder::MessageBuilder;

        // Collect attachment data first (before builder chain)
        let mut att_data: Vec<(String, Vec<u8>)> = Vec::new();
        if let Some(files) = attachments {
            for path in files {
                let file_path = std::path::Path::new(path);
                let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("attachment").to_string();
                let content = std::fs::read(file_path)?;
                att_data.push((filename, content));
            }
        }

        // Build using chaining exactly like the docs example
        let mut builder = MessageBuilder::new()
            .from(from)
            .to(to)
            .subject(subject)
            .text_body(body);

        // Add HTML body
        if let Some(h) = html {
            builder = builder.html_body(h);
        }

        // Add CC
        if let Some(cc_addrs) = cc {
            for addr in cc_addrs.split(',') {
                builder = builder.cc(addr.trim());
            }
        }

        // Add BCC
        if let Some(bcc_addrs) = bcc {
            for addr in bcc_addrs.split(',') {
                builder = builder.bcc(addr.trim());
            }
        }

        // Add attachments
        for (filename, content) in &att_data {
            builder = builder.attachment("application/octet-stream", filename.as_str(), content.as_slice());
        }

        let bytes = builder.write_to_vec()?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    fn guess_mime(path: &std::path::Path) -> &'static str {
        match path.extension().and_then(|e| e.to_str()) {
            Some("pdf") => "application/pdf",
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("zip") => "application/zip",
            Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            _ => "application/octet-stream",
        }
    }

    async fn send_smtp_raw(&self, host: &str, port: u16, username: &str, password: &str, from: &str, to: &str, raw_msg: &str) -> anyhow::Result<SendResult> {

        use tokio::net::TcpStream;
        use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};

        let tls_connector = {
            let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            tokio_rustls::TlsConnector::from(std::sync::Arc::new(config))
        };
        let domain = rustls_pki_types::ServerName::try_from(host)?.to_owned();

        let tcp = TcpStream::connect(format!("{host}:{port}")).await?;

        if port == 587 || port == 25 {
            // STARTTLS: connect plain, negotiate, then upgrade
            let (reader, mut writer) = tokio::io::split(tcp);
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            reader.read_line(&mut line).await?; line.clear();
            writer.write_all(b"EHLO mcp-email\r\n").await?;
            loop { line.clear(); reader.read_line(&mut line).await?; if line.starts_with("250 ") { break; } }
            writer.write_all(b"STARTTLS\r\n").await?;
            line.clear(); reader.read_line(&mut line).await?;
            if !line.starts_with("220") {
                anyhow::bail!("STARTTLS rejected: {}", line.trim());
            }

            // Reassemble the TcpStream from split halves
            let tcp_reassembled = reader.into_inner().unsplit(writer);
            let tls = tls_connector.connect(domain, tcp_reassembled).await?;
            let (tls_reader, mut tls_writer) = tokio::io::split(tls);
            let mut tls_reader = BufReader::new(tls_reader);
            let mut line = String::new();

            // Re-EHLO after TLS
            tls_writer.write_all(b"EHLO mcp-email\r\n").await?;
            loop { line.clear(); tls_reader.read_line(&mut line).await?; if line.starts_with("250 ") { break; } }

            Self::smtp_auth_and_send(&mut tls_reader, &mut tls_writer, username, password, from, to, &raw_msg).await
        } else {
            // Port 465: implicit TLS
            let tls = tls_connector.connect(domain, tcp).await?;
            let (tls_reader, mut tls_writer) = tokio::io::split(tls);
            let mut tls_reader = BufReader::new(tls_reader);
            let mut line = String::new();

            tls_reader.read_line(&mut line).await?; line.clear();
            tls_writer.write_all(b"EHLO mcp-email\r\n").await?;
            loop { line.clear(); tls_reader.read_line(&mut line).await?; if line.starts_with("250 ") { break; } }

            Self::smtp_auth_and_send(&mut tls_reader, &mut tls_writer, username, password, from, to, &raw_msg).await
        }
    }

    async fn smtp_auth_and_send<R: tokio::io::AsyncRead + Unpin, W: tokio::io::AsyncWrite + Unpin>(
        reader: &mut tokio::io::BufReader<R>, writer: &mut W,
        username: &str, password: &str, from: &str, to: &str, raw_msg: &str,
    ) -> anyhow::Result<SendResult> {
        use tokio::io::{AsyncWriteExt, AsyncBufReadExt};
        let mut line = String::new();

        writer.write_all(b"AUTH LOGIN\r\n").await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(format!("{}\r\n", base64_encode(username.as_bytes())).as_bytes()).await?;
        line.clear(); reader.read_line(&mut line).await?;
        writer.write_all(format!("{}\r\n", base64_encode(password.as_bytes())).as_bytes()).await?;
        line.clear(); reader.read_line(&mut line).await?;
        if !line.starts_with("235") {
            anyhow::bail!("SMTP auth failed: {}", line.trim());
        }

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

    async fn send_ses(&self, region: &str, _access_key: &str, _secret_key: &str, from: &str, to: &str, subject: &str, body: &str, _html: Option<&str>) -> anyhow::Result<SendResult> {
        let input = serde_json::json!({
            "Source": from,
            "Destination": {"ToAddresses": [to]},
            "Message": {
                "Subject": {"Data": subject},
                "Body": {"Text": {"Data": body}}
            }
        });
        let input_str = serde_json::to_string(&input)?;
        let output = tokio::process::Command::new("aws")
            .args(["ses", "send-email", "--region", region, "--cli-input-json", &input_str])
            .output().await?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let msg_id = text.split("MessageId").nth(1)
                .and_then(|s| s.split('"').nth(2))
                .unwrap_or("ses-sent").to_string();
            Ok(SendResult { message_id: msg_id, backend: "ses".into() })
        } else {
            anyhow::bail!("SES error: {}", String::from_utf8_lossy(&output.stderr))
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
        self.send_gmail_raw(token, &raw).await
    }

    async fn send_gmail_raw(&self, token: &str, raw_msg: &str) -> anyhow::Result<SendResult> {
        use base64ct::{Base64UrlUnpadded, Encoding};
        let encoded = Base64UrlUnpadded::encode_string(raw_msg.as_bytes());
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

    async fn imap_store_flag(&self, message_id: &str, action: &str, flag: &str) -> anyhow::Result<()> {
        let (host, port, username, password) = match &self.read_backend {
            Some(ReadBackend::Imap { host, port, username, password }) => (host.clone(), *port, username.clone(), password.clone()),
            _ => anyhow::bail!("Not IMAP backend"),
        };
        use tokio_util::compat::TokioAsyncReadCompatExt;
        let tcp = tokio::net::TcpStream::connect(format!("{host}:{port}")).await?;
        let tls = {
            let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = tokio_rustls::rustls::ClientConfig::builder().with_root_certificates(root_store).with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
            let domain = rustls_pki_types::ServerName::try_from(host.as_str())?.to_owned();
            connector.connect(domain, tcp).await?
        };
        let mut client = async_imap::Client::new(tls.compat());
        let _greeting = client.read_response().await;
        let mut session = client.login(&username, &password).await.map_err(|e| anyhow::anyhow!("{}", e.0))?;
        session.select("INBOX").await?;
        session.store(message_id, format!("{action} ({flag})")).await?;
        session.logout().await?;
        Ok(())
    }

    async fn imap_move(&self, message_id: &str, folder: &str) -> anyhow::Result<()> {
        let (host, port, username, password) = match &self.read_backend {
            Some(ReadBackend::Imap { host, port, username, password }) => (host.clone(), *port, username.clone(), password.clone()),
            _ => anyhow::bail!("Not IMAP backend"),
        };
        use tokio_util::compat::TokioAsyncReadCompatExt;
        let tcp = tokio::net::TcpStream::connect(format!("{host}:{port}")).await?;
        let tls = {
            let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = tokio_rustls::rustls::ClientConfig::builder().with_root_certificates(root_store).with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
            let domain = rustls_pki_types::ServerName::try_from(host.as_str())?.to_owned();
            connector.connect(domain, tcp).await?
        };
        let mut client = async_imap::Client::new(tls.compat());
        let _greeting = client.read_response().await;
        let mut session = client.login(&username, &password).await.map_err(|e| anyhow::anyhow!("{}", e.0))?;
        session.select("INBOX").await?;
        session.mv(message_id, folder).await?;
        session.logout().await?;
        Ok(())
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
            let encoded = {
                use base64ct::{Base64UrlUnpadded, Encoding};
                Base64UrlUnpadded::encode_string(raw.as_bytes())
            };
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
        if self.is_imap() {
            return self.imap_move(message_id, folder).await;
        }
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
        if self.is_imap() {
            return self.imap_store_flag(message_id, "+FLAGS", "\\Seen").await;
        }
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

    pub async fn forward_email(&self, message_id: &str, to: &str) -> anyhow::Result<String> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let orig: serde_json::Value = self.http
                .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"))
                .bearer_auth(token).query(&[("format", "full")])
                .send().await?.error_for_status()?.json().await?;
            let snippet = orig["snippet"].as_str().unwrap_or("");
            let subject = format!("Fwd: {}", orig["payload"]["headers"].as_array()
                .and_then(|h| h.iter().find(|x| x["name"].as_str() == Some("Subject")))
                .and_then(|x| x["value"].as_str()).unwrap_or(""));
            let raw = format!("To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n---------- Forwarded message ----------\r\n{snippet}");
            let encoded = base64_url_encode(raw.as_bytes());
            let resp: serde_json::Value = self.http
                .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
                .bearer_auth(token).json(&serde_json::json!({"raw": encoded}))
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["id"].as_str().unwrap_or("sent").to_string())
        } else {
            self.http.post(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}/forward"))
                .bearer_auth(token)
                .json(&serde_json::json!({"toRecipients": [{"emailAddress": {"address": to}}]}))
                .send().await?.error_for_status()?;
            Ok("forwarded".to_string())
        }
    }

    pub async fn create_draft(&self, to: &str, subject: &str, body: &str, html: Option<&str>) -> anyhow::Result<String> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let content = if let Some(h) = html {
                format!("To: {to}\r\nSubject: {subject}\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{h}")
            } else {
                format!("To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}")
            };
            let encoded = base64_url_encode(content.as_bytes());
            let resp: serde_json::Value = self.http
                .post("https://gmail.googleapis.com/gmail/v1/users/me/drafts")
                .bearer_auth(token).json(&serde_json::json!({"message": {"raw": encoded}}))
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["id"].as_str().unwrap_or("created").to_string())
        } else {
            let msg = serde_json::json!({
                "subject": subject,
                "body": {"contentType": if html.is_some() { "HTML" } else { "Text" }, "content": html.unwrap_or(body)},
                "toRecipients": [{"emailAddress": {"address": to}}]
            });
            let resp: serde_json::Value = self.http
                .post("https://graph.microsoft.com/v1.0/me/messages")
                .bearer_auth(token).json(&msg)
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["id"].as_str().unwrap_or("created").to_string())
        }
    }

    pub async fn list_drafts(&self, limit: u32) -> anyhow::Result<Vec<EmailMessage>> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get("https://gmail.googleapis.com/gmail/v1/users/me/drafts")
                .bearer_auth(token).query(&[("maxResults", limit.to_string())])
                .send().await?.error_for_status()?.json().await?;
            let mut msgs = Vec::new();
            if let Some(drafts) = resp["drafts"].as_array() {
                for d in drafts.iter().take(limit as usize) {
                    if let Some(id) = d["message"]["id"].as_str() {
                        if let Ok(m) = self.get_email(id).await { msgs.push(m); }
                    }
                }
            }
            Ok(msgs)
        } else {
            let resp: serde_json::Value = self.http
                .get("https://graph.microsoft.com/v1.0/me/mailFolders/drafts/messages")
                .bearer_auth(token).query(&[("$top", &limit.to_string())])
                .send().await?.error_for_status()?.json().await?;
            Ok(parse_ms_messages(&resp))
        }
    }

    pub async fn send_draft(&self, draft_id: &str) -> anyhow::Result<String> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .post(format!("https://gmail.googleapis.com/gmail/v1/users/me/drafts/send"))
                .bearer_auth(token).json(&serde_json::json!({"id": draft_id}))
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["id"].as_str().unwrap_or("sent").to_string())
        } else {
            self.http.post(format!("https://graph.microsoft.com/v1.0/me/messages/{draft_id}/send"))
                .bearer_auth(token).send().await?.error_for_status()?;
            Ok("sent".to_string())
        }
    }

    pub async fn delete_email(&self, message_id: &str, permanent: bool) -> anyhow::Result<()> {
        if self.is_imap() {
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
                let config = tokio_rustls::rustls::ClientConfig::builder().with_root_certificates(root_store).with_no_client_auth();
                let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
                let domain = rustls_pki_types::ServerName::try_from(host.as_str())?.to_owned();
                connector.connect(domain, tcp).await?
            };
            let mut client = async_imap::Client::new(tls.compat());
            let _greeting = client.read_response().await;
            let mut session = client.login(&username, &password).await.map_err(|e| anyhow::anyhow!("{}", e.0))?;
            session.select("INBOX").await?;
            let _: Vec<_> = session.store(message_id, "+FLAGS (\\Deleted)").await?.try_collect().await?;
            if permanent { let _: Vec<_> = session.expunge().await?.try_collect().await?; }
            session.logout().await?;
            return Ok(());
        }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            if permanent {
                self.http.delete(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"))
                    .bearer_auth(token).send().await?.error_for_status()?;
            } else {
                self.http.post(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/trash"))
                    .bearer_auth(token).send().await?.error_for_status()?;
            }
        } else {
            if permanent {
                self.http.delete(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}"))
                    .bearer_auth(token).send().await?.error_for_status()?;
            } else {
                self.http.post(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}/move"))
                    .bearer_auth(token).json(&serde_json::json!({"destinationId": "deleteditems"}))
                    .send().await?.error_for_status()?;
            }
        }
        Ok(())
    }

    pub async fn mark_unread(&self, message_id: &str) -> anyhow::Result<()> {
        if self.is_imap() {
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
                let config = tokio_rustls::rustls::ClientConfig::builder().with_root_certificates(root_store).with_no_client_auth();
                let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
                let domain = rustls_pki_types::ServerName::try_from(host.as_str())?.to_owned();
                connector.connect(domain, tcp).await?
            };
            let mut client = async_imap::Client::new(tls.compat());
            let _greeting = client.read_response().await;
            let mut session = client.login(&username, &password).await.map_err(|e| anyhow::anyhow!("{}", e.0))?;
            session.select("INBOX").await?;
            let _: Vec<_> = session.store(message_id, "-FLAGS (\\Seen)").await?.try_collect().await?;
            session.logout().await?;
            return Ok(());
        }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            self.http.post(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/modify"))
                .bearer_auth(token).json(&serde_json::json!({"addLabelIds": ["UNREAD"]}))
                .send().await?.error_for_status()?;
        } else {
            self.http.patch(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}"))
                .bearer_auth(token).json(&serde_json::json!({"isRead": false}))
                .send().await?.error_for_status()?;
        }
        Ok(())
    }

    pub async fn star_email(&self, message_id: &str) -> anyhow::Result<()> {
        if self.is_imap() {
            return self.imap_store_flag(message_id, "+FLAGS", "\\Flagged").await;
        }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            self.http.post(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/modify"))
                .bearer_auth(token).json(&serde_json::json!({"addLabelIds": ["STARRED"]}))
                .send().await?.error_for_status()?;
        } else {
            self.http.patch(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}"))
                .bearer_auth(token).json(&serde_json::json!({"flag": {"flagStatus": "flagged"}}))
                .send().await?.error_for_status()?;
        }
        Ok(())
    }

    pub async fn get_email_body(&self, message_id: &str) -> anyhow::Result<String> {
        if self.is_imap() {
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
                let config = tokio_rustls::rustls::ClientConfig::builder().with_root_certificates(root_store).with_no_client_auth();
                let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
                let domain = rustls_pki_types::ServerName::try_from(host.as_str())?.to_owned();
                connector.connect(domain, tcp).await?
            };
            let mut client = async_imap::Client::new(tls.compat());
            let _greeting = client.read_response().await;
            let mut session = client.login(&username, &password).await.map_err(|e| anyhow::anyhow!("{}", e.0))?;
            session.select("INBOX").await?;
            let messages: Vec<_> = session.fetch(message_id, "BODY[]").await?.try_collect().await?;
            let body = messages.first().and_then(|m| m.body()).map(|b| String::from_utf8_lossy(b).to_string()).unwrap_or_default();
            session.logout().await?;
            return Ok(body);
        }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"))
                .bearer_auth(token).query(&[("format", "full")])
                .send().await?.error_for_status()?.json().await?;
            // Try to find text/html or text/plain part
            if let Some(parts) = resp["payload"]["parts"].as_array() {
                for p in parts {
                    if p["mimeType"].as_str() == Some("text/html") || p["mimeType"].as_str() == Some("text/plain") {
                        if let Some(data) = p["body"]["data"].as_str() {
                            return Ok(String::from_utf8_lossy(&base64_url_decode(data)).to_string());
                        }
                    }
                }
            }
            // Single-part message
            if let Some(data) = resp["payload"]["body"]["data"].as_str() {
                return Ok(String::from_utf8_lossy(&base64_url_decode(data)).to_string());
            }
            Ok(resp["snippet"].as_str().unwrap_or("").to_string())
        } else {
            let resp: serde_json::Value = self.http
                .get(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}"))
                .bearer_auth(token).query(&[("$select", "body")])
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["body"]["content"].as_str().unwrap_or("").to_string())
        }
    }

    pub async fn get_thread(&self, message_id: &str) -> anyhow::Result<Vec<EmailMessage>> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            // Get message to find threadId
            let msg: serde_json::Value = self.http
                .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"))
                .bearer_auth(token).query(&[("format", "metadata")])
                .send().await?.error_for_status()?.json().await?;
            let thread_id = msg["threadId"].as_str().unwrap_or(message_id);
            let resp: serde_json::Value = self.http
                .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/threads/{thread_id}"))
                .bearer_auth(token).query(&[("format", "metadata")])
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["messages"].as_array()
                .map(|a| a.iter().map(|m| parse_gmail_message(m)).collect())
                .unwrap_or_default())
        } else {
            // Microsoft: get conversationId then filter
            let msg: serde_json::Value = self.http
                .get(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}"))
                .bearer_auth(token).query(&[("$select", "conversationId")])
                .send().await?.error_for_status()?.json().await?;
            let conv_id = msg["conversationId"].as_str().unwrap_or("");
            let resp: serde_json::Value = self.http
                .get("https://graph.microsoft.com/v1.0/me/messages")
                .bearer_auth(token).query(&[("$filter", &format!("conversationId eq '{conv_id}'")), ("$top", &"50".to_string())])
                .send().await?.error_for_status()?.json().await?;
            Ok(parse_ms_messages(&resp))
        }
    }

    pub async fn download_attachment(&self, message_id: &str, attachment_id: &str) -> anyhow::Result<String> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/attachments/{attachment_id}"))
                .bearer_auth(token).send().await?.error_for_status()?.json().await?;
            Ok(resp["data"].as_str().unwrap_or("").to_string())
        } else {
            let resp: serde_json::Value = self.http
                .get(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}/attachments/{attachment_id}"))
                .bearer_auth(token).send().await?.error_for_status()?.json().await?;
            Ok(resp["contentBytes"].as_str().unwrap_or("").to_string())
        }
    }

    pub async fn create_label(&self, name: &str) -> anyhow::Result<String> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let resp: serde_json::Value = self.http
                .post("https://gmail.googleapis.com/gmail/v1/users/me/labels")
                .bearer_auth(token).json(&serde_json::json!({"name": name}))
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["id"].as_str().unwrap_or("created").to_string())
        } else {
            let resp: serde_json::Value = self.http
                .post("https://graph.microsoft.com/v1.0/me/mailFolders")
                .bearer_auth(token).json(&serde_json::json!({"displayName": name}))
                .send().await?.error_for_status()?.json().await?;
            Ok(resp["id"].as_str().unwrap_or("created").to_string())
        }
    }

    pub async fn delete_label(&self, label_id: &str) -> anyhow::Result<()> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            self.http.delete(format!("https://gmail.googleapis.com/gmail/v1/users/me/labels/{label_id}"))
                .bearer_auth(token).send().await?.error_for_status()?;
        } else {
            self.http.delete(format!("https://graph.microsoft.com/v1.0/me/mailFolders/{label_id}"))
                .bearer_auth(token).send().await?.error_for_status()?;
        }
        Ok(())
    }

    pub async fn batch_delete(&self, message_ids: &[String], permanent: bool) -> anyhow::Result<String> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            if permanent {
                self.http.post("https://gmail.googleapis.com/gmail/v1/users/me/messages/batchDelete")
                    .bearer_auth(token).json(&serde_json::json!({"ids": message_ids}))
                    .send().await?.error_for_status()?;
            } else {
                self.http.post("https://gmail.googleapis.com/gmail/v1/users/me/messages/batchModify")
                    .bearer_auth(token).json(&serde_json::json!({"ids": message_ids, "addLabelIds": ["TRASH"]}))
                    .send().await?.error_for_status()?;
            }
        } else {
            for id in message_ids {
                self.delete_email(id, permanent).await?;
            }
        }
        Ok(format!("Deleted {} messages", message_ids.len()))
    }

    pub async fn batch_move(&self, message_ids: &[String], folder: &str) -> anyhow::Result<String> {
        if self.is_imap() { anyhow::bail!("Not supported on IMAP"); }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            self.http.post("https://gmail.googleapis.com/gmail/v1/users/me/messages/batchModify")
                .bearer_auth(token).json(&serde_json::json!({"ids": message_ids, "addLabelIds": [folder]}))
                .send().await?.error_for_status()?;
        } else {
            for id in message_ids {
                self.move_to_folder(id, folder).await?;
            }
        }
        Ok(format!("Moved {} messages to {folder}", message_ids.len()))
    }

    pub async fn batch_mark(&self, message_ids: &[String], read: bool) -> anyhow::Result<String> {
        if self.is_imap() {
            for id in message_ids {
                if read {
                    self.imap_store_flag(id, "+FLAGS", "\\Seen").await?;
                } else {
                    self.imap_store_flag(id, "-FLAGS", "\\Seen").await?;
                }
            }
            return Ok(format!("Marked {} messages", message_ids.len()));
        }
        let (token, is_gmail) = self.read_token()?;
        if is_gmail {
            let body = if read {
                serde_json::json!({"ids": message_ids, "removeLabelIds": ["UNREAD"]})
            } else {
                serde_json::json!({"ids": message_ids, "addLabelIds": ["UNREAD"]})
            };
            self.http.post("https://gmail.googleapis.com/gmail/v1/users/me/messages/batchModify")
                .bearer_auth(token).json(&body)
                .send().await?.error_for_status()?;
        } else {
            for id in message_ids {
                if read { self.mark_read(id).await?; } else { self.mark_unread(id).await?; }
            }
        }
        Ok(format!("Marked {} messages as {}", message_ids.len(), if read { "read" } else { "unread" }))
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

fn base64_url_decode(data: &str) -> Vec<u8> {
    let s = data.replace('-', "+").replace('_', "/");
    let padded = match s.len() % 4 {
        2 => format!("{s}=="),
        3 => format!("{s}="),
        _ => s,
    };
    let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let bytes: Vec<u8> = padded.bytes().filter(|&b| b != b'=').map(|b| {
        chars.iter().position(|&c| c == b).unwrap_or(0) as u8
    }).collect();
    for chunk in bytes.chunks(4) {
        if chunk.len() >= 2 {
            result.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            result.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() >= 4 {
            result.push((chunk[2] << 6) | chunk[3]);
        }
    }
    result
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