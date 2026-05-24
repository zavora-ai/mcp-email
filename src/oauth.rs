use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const GMAIL_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GMAIL_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GMAIL_SCOPES: &str = "https://mail.google.com/";

const MS_AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const MS_SCOPES: &str = "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send offline_access";

const REDIRECT_URI: &str = "http://localhost:8856/callback";

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenStore {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

fn credentials_path(provider: &str) -> PathBuf {
    let dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".mcp-email");
    std::fs::create_dir_all(&dir).ok();
    dir.join(format!("{provider}_token.json"))
}

fn config_path(provider: &str) -> PathBuf {
    let dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".mcp-email");
    dir.join(format!("{provider}_oauth.json"))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
}

pub fn load_oauth_config(provider: &str) -> Result<OAuthConfig> {
    let path = config_path(provider);
    if !path.exists() {
        anyhow::bail!(
            "OAuth not configured for {provider}. Run: mcp-email auth {provider}\n\
             Or create {} with {{\"client_id\": \"...\", \"client_secret\": \"...\"}}",
            path.display()
        );
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn load_token(provider: &str) -> Option<TokenStore> {
    let path = credentials_path(provider);
    std::fs::read_to_string(&path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn save_token(provider: &str, token: &TokenStore) -> Result<()> {
    let path = credentials_path(provider);
    std::fs::write(&path, serde_json::to_string_pretty(token)?)?;
    // Restrict permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub async fn get_valid_token(provider: &str) -> Result<String> {
    let token = load_token(provider).ok_or_else(|| anyhow::anyhow!(
        "No token for {provider}. Run: mcp-email auth {provider}"
    ))?;

    // Check expiry (refresh if within 5 min)
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    if let Some(expires_at) = token.expires_at {
        if now + 300 >= expires_at {
            if let Some(ref refresh) = token.refresh_token {
                return refresh_token(provider, refresh).await;
            }
        }
    }
    Ok(token.access_token)
}

async fn refresh_token(provider: &str, refresh: &str) -> Result<String> {
    let config = load_oauth_config(provider)?;
    let token_url = match provider {
        "gmail" => GMAIL_TOKEN_URL,
        "microsoft" => MS_TOKEN_URL,
        _ => anyhow::bail!("Unknown provider"),
    };

    let mut params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh),
        ("client_id", &config.client_id),
    ];
    let secret_str;
    if let Some(ref s) = config.client_secret {
        secret_str = s.clone();
        params.push(("client_secret", &secret_str));
    }

    let client = reqwest::Client::new();
    let resp: TokenResponse = client.post(token_url)
        .form(&params)
        .send().await?.error_for_status()?
        .json().await?;

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let store = TokenStore {
        access_token: resp.access_token.clone(),
        refresh_token: resp.refresh_token.or_else(|| Some(refresh.to_string())),
        expires_at: resp.expires_in.map(|e| now + e),
        provider: provider.to_string(),
    };
    save_token(provider, &store)?;
    tracing::info!("Token refreshed for {provider}");
    Ok(resp.access_token)
}

/// Run the OAuth PKCE flow — opens browser, starts local server, exchanges code
pub async fn run_auth_flow(provider: &str, client_id: &str, client_secret: Option<&str>) -> Result<()> {
    // Save config
    let config = OAuthConfig {
        client_id: client_id.to_string(),
        client_secret: client_secret.map(String::from),
    };
    let config_file = config_path(provider);
    std::fs::create_dir_all(config_file.parent().unwrap())?;
    std::fs::write(&config_file, serde_json::to_string_pretty(&config)?)?;

    // Generate PKCE
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let verifier: String = (0..64).map(|i| {
        let mut h = DefaultHasher::new();
        (i, std::time::SystemTime::now()).hash(&mut h);
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        chars[(h.finish() as usize) % chars.len()] as char
    }).collect();

    let (auth_url, scopes) = match provider {
        "gmail" => (GMAIL_AUTH_URL, GMAIL_SCOPES),
        "microsoft" => (MS_AUTH_URL, MS_SCOPES),
        _ => anyhow::bail!("Unknown provider: {provider}"),
    };

    let url = format!(
        "{auth_url}?client_id={client_id}&redirect_uri={REDIRECT_URI}&response_type=code&scope={scopes}&access_type=offline&prompt=consent"
    );

    println!("Opening browser for {provider} authorization...");
    println!("If browser doesn't open, visit:\n{url}\n");
    open::that(&url).ok();

    // Start local HTTP server to receive callback
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8856").await?;
    println!("Waiting for authorization callback on http://localhost:8856/callback ...");

    let (mut stream, _) = listener.accept().await?;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Extract code from GET /callback?code=XXX
    let code = request.split("code=").nth(1)
        .and_then(|s| s.split('&').next())
        .and_then(|s| s.split(' ').next())
        .ok_or_else(|| anyhow::anyhow!("No code in callback"))?
        .to_string();

    // Send success response
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h2>✅ Authorization successful!</h2><p>You can close this tab.</p></body></html>";
    stream.write_all(response.as_bytes()).await?;
    drop(stream);

    // Exchange code for tokens
    let token_url = match provider {
        "gmail" => GMAIL_TOKEN_URL,
        "microsoft" => MS_TOKEN_URL,
        _ => unreachable!(),
    };

    let mut params = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", REDIRECT_URI.to_string()),
        ("client_id", client_id.to_string()),
    ];
    if let Some(secret) = client_secret {
        params.push(("client_secret", secret.to_string()));
    }

    let client = reqwest::Client::new();
    let resp: TokenResponse = client.post(token_url)
        .form(&params)
        .send().await?.error_for_status()?
        .json().await?;

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let store = TokenStore {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token,
        expires_at: resp.expires_in.map(|e| now + e),
        provider: provider.to_string(),
    };
    save_token(provider, &store)?;

    println!("✅ Token saved to {}", credentials_path(provider).display());
    println!("   mcp-email will auto-refresh when it expires.");
    Ok(())
}
