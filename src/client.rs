use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub enum EmailProvider {
    Gmail { token: String },
    Microsoft { token: String },
}

#[derive(Clone)]
pub struct EmailClient {
    http: Client,
    pub provider: EmailProvider,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
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

impl EmailClient {
    pub fn new(provider: EmailProvider) -> Self {
        Self { http: Client::new(), provider }
    }

    fn token(&self) -> &str {
        match &self.provider {
            EmailProvider::Gmail { token } | EmailProvider::Microsoft { token } => token,
        }
    }

    pub async fn send_email(&self, to: &str, subject: &str, body: &str) -> anyhow::Result<String> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                let raw = format!("To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}");
                let encoded = base64_url_encode(raw.as_bytes());
                let resp: serde_json::Value = self.http
                    .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
                    .bearer_auth(self.token())
                    .json(&serde_json::json!({"raw": encoded}))
                    .send().await?.error_for_status()?.json().await?;
                Ok(resp["id"].as_str().unwrap_or("sent").to_string())
            }
            EmailProvider::Microsoft { .. } => {
                let msg = serde_json::json!({
                    "message": {
                        "subject": subject,
                        "body": {"contentType": "Text", "content": body},
                        "toRecipients": [{"emailAddress": {"address": to}}]
                    }
                });
                self.http.post("https://graph.microsoft.com/v1.0/me/sendMail")
                    .bearer_auth(self.token()).json(&msg)
                    .send().await?.error_for_status()?;
                Ok("sent".to_string())
            }
        }
    }

    pub async fn list_inbox(&self, limit: u32) -> anyhow::Result<Vec<EmailMessage>> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                let resp: serde_json::Value = self.http
                    .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
                    .bearer_auth(self.token())
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
            }
            EmailProvider::Microsoft { .. } => {
                let resp: serde_json::Value = self.http
                    .get("https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages")
                    .bearer_auth(self.token())
                    .query(&[("$top", &limit.to_string())])
                    .send().await?.error_for_status()?.json().await?;
                Ok(parse_ms_messages(&resp))
            }
        }
    }

    pub async fn get_email(&self, id: &str) -> anyhow::Result<EmailMessage> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                let resp: serde_json::Value = self.http
                    .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}"))
                    .bearer_auth(self.token())
                    .query(&[("format", "metadata")])
                    .send().await?.error_for_status()?.json().await?;
                Ok(parse_gmail_message(&resp))
            }
            EmailProvider::Microsoft { .. } => {
                let resp: serde_json::Value = self.http
                    .get(format!("https://graph.microsoft.com/v1.0/me/messages/{id}"))
                    .bearer_auth(self.token())
                    .send().await?.error_for_status()?.json().await?;
                Ok(EmailMessage {
                    id: id.to_string(),
                    subject: resp["subject"].as_str().map(String::from),
                    from: resp["from"]["emailAddress"]["address"].as_str().map(String::from),
                    snippet: resp["bodyPreview"].as_str().map(String::from),
                    date: resp["receivedDateTime"].as_str().map(String::from),
                    is_read: resp["isRead"].as_bool(),
                })
            }
        }
    }

    pub async fn search_emails(&self, query: &str, limit: u32) -> anyhow::Result<Vec<EmailMessage>> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                let resp: serde_json::Value = self.http
                    .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
                    .bearer_auth(self.token())
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
            }
            EmailProvider::Microsoft { .. } => {
                let resp: serde_json::Value = self.http
                    .get("https://graph.microsoft.com/v1.0/me/messages")
                    .bearer_auth(self.token())
                    .query(&[("$search", &format!("\"{query}\"")), ("$top", &limit.to_string())])
                    .send().await?.error_for_status()?.json().await?;
                Ok(parse_ms_messages(&resp))
            }
        }
    }

    pub async fn reply_to_email(&self, id: &str, body: &str) -> anyhow::Result<String> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                let orig = self.get_email(id).await?;
                let subject = format!("Re: {}", orig.subject.unwrap_or_default());
                let to = orig.from.unwrap_or_default();
                let raw = format!("To: {to}\r\nSubject: {subject}\r\nIn-Reply-To: {id}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}");
                let encoded = base64_url_encode(raw.as_bytes());
                let resp: serde_json::Value = self.http
                    .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
                    .bearer_auth(self.token())
                    .json(&serde_json::json!({"raw": encoded, "threadId": id}))
                    .send().await?.error_for_status()?.json().await?;
                Ok(resp["id"].as_str().unwrap_or("sent").to_string())
            }
            EmailProvider::Microsoft { .. } => {
                self.http.post(format!("https://graph.microsoft.com/v1.0/me/messages/{id}/reply"))
                    .bearer_auth(self.token())
                    .json(&serde_json::json!({"comment": body}))
                    .send().await?.error_for_status()?;
                Ok("replied".to_string())
            }
        }
    }

    pub async fn list_labels(&self) -> anyhow::Result<Vec<Label>> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                let resp: serde_json::Value = self.http
                    .get("https://gmail.googleapis.com/gmail/v1/users/me/labels")
                    .bearer_auth(self.token())
                    .send().await?.error_for_status()?.json().await?;
                Ok(resp["labels"].as_array()
                    .map(|a| a.iter().map(|l| Label { id: l["id"].as_str().unwrap_or("").into(), name: l["name"].as_str().unwrap_or("").into() }).collect())
                    .unwrap_or_default())
            }
            EmailProvider::Microsoft { .. } => {
                let resp: serde_json::Value = self.http
                    .get("https://graph.microsoft.com/v1.0/me/mailFolders")
                    .bearer_auth(self.token())
                    .send().await?.error_for_status()?.json().await?;
                Ok(resp["value"].as_array()
                    .map(|a| a.iter().map(|f| Label { id: f["id"].as_str().unwrap_or("").into(), name: f["displayName"].as_str().unwrap_or("").into() }).collect())
                    .unwrap_or_default())
            }
        }
    }

    pub async fn move_to_folder(&self, message_id: &str, folder: &str) -> anyhow::Result<()> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                self.http.post(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/modify"))
                    .bearer_auth(self.token())
                    .json(&serde_json::json!({"addLabelIds": [folder]}))
                    .send().await?.error_for_status()?;
            }
            EmailProvider::Microsoft { .. } => {
                self.http.post(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}/move"))
                    .bearer_auth(self.token())
                    .json(&serde_json::json!({"destinationId": folder}))
                    .send().await?.error_for_status()?;
            }
        }
        Ok(())
    }

    pub async fn mark_read(&self, message_id: &str) -> anyhow::Result<()> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                self.http.post(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}/modify"))
                    .bearer_auth(self.token())
                    .json(&serde_json::json!({"removeLabelIds": ["UNREAD"]}))
                    .send().await?.error_for_status()?;
            }
            EmailProvider::Microsoft { .. } => {
                self.http.patch(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}"))
                    .bearer_auth(self.token())
                    .json(&serde_json::json!({"isRead": true}))
                    .send().await?.error_for_status()?;
            }
        }
        Ok(())
    }

    pub async fn get_attachments(&self, message_id: &str) -> anyhow::Result<Vec<Attachment>> {
        match &self.provider {
            EmailProvider::Gmail { .. } => {
                let resp: serde_json::Value = self.http
                    .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"))
                    .bearer_auth(self.token())
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
            }
            EmailProvider::Microsoft { .. } => {
                let resp: serde_json::Value = self.http
                    .get(format!("https://graph.microsoft.com/v1.0/me/messages/{message_id}/attachments"))
                    .bearer_auth(self.token())
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
    }
}

fn base64_url_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 { result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char); } else { result.push('='); }
        if chunk.len() > 2 { result.push(CHARS[(triple & 0x3F) as usize] as char); } else { result.push('='); }
    }
    result.replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
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
            snippet: m["bodyPreview"].as_str().map(String::from),
            date: m["receivedDateTime"].as_str().map(String::from),
            is_read: m["isRead"].as_bool(),
        }).collect())
        .unwrap_or_default()
}
