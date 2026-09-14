use rmcp::{
    model::{Implementation, ServerCapabilities, ServerInfo},
    ServerHandler,
};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct GmailService;

#[path = "gmail_models.rs"]
mod models;
#[path = "gmail_tools.rs"]
mod tools;
pub use models::*;
impl GmailService {
    fn resolve_credentials() -> Result<(String, String), String> {
        let mut email = env::var("GMAIL_EMAIL")
            .unwrap_or_default()
            .trim()
            .to_string();
        let mut app_password = env::var("GMAIL_APP_PASSWORD")
            .unwrap_or_default()
            .trim()
            .to_string();

        if let Ok(home) = env::var("HOME") {
            let path = PathBuf::from(home).join(".config/credentials/workspace-google");
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(val) = trimmed.strip_prefix("GMAIL_EMAIL=") {
                            if email.is_empty() {
                                email = val.trim().to_string();
                            }
                        }
                        if let Some(val) = trimmed.strip_prefix("GMAIL_APP_PASSWORD=") {
                            if app_password.is_empty() {
                                app_password = val.trim().to_string();
                            }
                        }
                    }
                }
            }
        }

        let clean_password = app_password.replace(' ', "").trim().to_string();

        if email.is_empty() {
            return Err("Gmail email address not configured. Please use tool `gmail_set_credentials` or run `smcp setup gmail`.".into());
        }

        if clean_password.is_empty() {
            return Err("Gmail app password not configured. Please use tool `gmail_set_credentials` or run `smcp setup gmail`.".into());
        }

        Ok((email, clean_password))
    }

    fn test_imap_login(email: &str, pass: &str) -> Result<(), String> {
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| format!("TLS build failed: {e}"))?;

        let client = imap::connect(("imap.gmail.com", 993), "imap.gmail.com", &tls)
            .map_err(|e| format!("IMAP connect failed: {e}"))?;

        let mut session = client.login(email, pass).map_err(|(e, _)| {
            format!("IMAP authentication failed: {e}. Check your App Password.")
        })?;

        let _ = session.logout();
        Ok(())
    }

    fn connect_imap_sync(
    ) -> Result<imap::Session<native_tls::TlsStream<std::net::TcpStream>>, String> {
        let (email, pass) = Self::resolve_credentials()?;
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| format!("TLS build failed: {e}"))?;

        let client = imap::connect(("imap.gmail.com", 993), "imap.gmail.com", &tls)
            .map_err(|e| format!("IMAP connect failed: {e}"))?;

        let session = client
            .login(&email, &pass)
            .map_err(|(e, _)| format!("IMAP login failed: {e}"))?;

        Ok(session)
    }

    pub fn resolve_mailbox_name(folder: Option<&str>) -> &'static str {
        match folder.unwrap_or("inbox").to_lowercase().as_str() {
            "spam" | "junk" => "[Gmail]/Spam",
            "trash" | "bin" => "[Gmail]/Trash",
            "sent" | "sentmail" | "sent_mail" => "[Gmail]/Sent Mail",
            "drafts" | "draft" => "[Gmail]/Drafts",
            "all" | "allmail" | "all_mail" => "[Gmail]/All Mail",
            "starred" => "[Gmail]/Starred",
            "important" => "[Gmail]/Important",
            _ => "INBOX",
        }
    }

    fn detect_sender_name(
        session: &mut imap::Session<native_tls::TlsStream<std::net::TcpStream>>,
        my_email: &str,
    ) -> Option<String> {
        let sent_folders = ["[Gmail]/Sent Mail", "INBOX"];
        for folder in sent_folders {
            if session.select(folder).is_ok() {
                if let Ok(seqs) = session.search(format!("FROM \"{my_email}\"")) {
                    if let Some(&last_id) = seqs.iter().max() {
                        if let Ok(msgs) =
                            session.fetch(last_id.to_string(), "(BODY.PEEK[HEADER.FIELDS (FROM)])")
                        {
                            for m in &msgs {
                                if let Some(hdr) = m.header() {
                                    if let Ok((headers, _)) = mailparse::parse_headers(hdr) {
                                        for h in headers {
                                            if h.get_key().eq_ignore_ascii_case("from") {
                                                let val = h.get_value();
                                                if let Some(idx) = val.find('<') {
                                                    let name =
                                                        val[..idx].trim().trim_matches('"').trim();
                                                    if !name.is_empty() {
                                                        return Some(name.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

fn mime_type_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
    }
}
