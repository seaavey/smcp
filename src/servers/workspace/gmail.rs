use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use rmcp::{
    model::{Implementation, ServerCapabilities, ServerInfo},
    ServerHandler, tool,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Default)]
pub struct GmailService;

#[derive(Deserialize, JsonSchema)]
pub struct CheckEmailsParam {
    #[schemars(description = "Maximum number of recent emails to retrieve (default: 10, max: 30)")]
    pub limit: Option<u32>,
    #[schemars(description = "Optional filter: 'all' or 'unread' (default: 'unread')")]
    pub filter: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadEmailParam {
    #[schemars(description = "The numeric sequence ID of the email from check_emails")]
    pub id: u32,
}

#[derive(Deserialize, JsonSchema)]
pub struct SendEmailParam {
    #[schemars(description = "Recipient email address")]
    pub to: String,
    #[schemars(description = "Email subject")]
    pub subject: String,
    #[schemars(description = "Email body text")]
    pub body: String,
    #[schemars(description = "Optional sender display name (default: 'Muhammad Adriansyah')")]
    pub from_name: Option<String>,
}

impl GmailService {
    fn get_credentials() -> Result<(String, String), String> {
        let email = "seaavey@gmail.com".to_string();
        let pwd_path = "/root/.config/credentials/gmail_app_password";
        let raw = fs::read_to_string(pwd_path)
            .map_err(|e| format!("Failed to read gmail app password: {e}"))?;
        let pass = raw.replace(' ', "").trim().to_string();
        Ok((email, pass))
    }

    fn connect_imap_sync() -> Result<imap::Session<native_tls::TlsStream<std::net::TcpStream>>, String> {
        let (email, pass) = Self::get_credentials()?;
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
}

#[tool(tool_box)]
impl GmailService {
    #[tool(name = "gmail_check_emails", description = "Check recent emails in INBOX (returns ID, date, from, and subject)")]
    pub async fn check_emails(&self, #[tool(aggr)] param: CheckEmailsParam) -> String {
        tokio::task::spawn_blocking(move || {
            let mut session = match Self::connect_imap_sync() {
                Ok(s) => s,
                Err(e) => return format!("{{\"error\": \"{e}\"}}"),
            };

            if let Err(e) = session.select("INBOX") {
                return format!("{{\"error\": \"Failed to select INBOX: {e}\"}}");
            }

            let filter = param.filter.unwrap_or_else(|| "unread".into());
            let query = if filter.to_lowercase() == "all" {
                "ALL"
            } else {
                "UNSEEN"
            };

            let seq_set = match session.search(query) {
                Ok(set) => set,
                Err(e) => return format!("{{\"error\": \"Failed to search emails: {e}\"}}"),
            };

            if seq_set.is_empty() {
                let _ = session.logout();
                return serde_json::json!({
                    "message": format!("No emails found matching query: {query}"),
                    "emails": []
                }).to_string();
            }

            let mut seq_vec: Vec<u32> = seq_set.into_iter().collect();
            seq_vec.sort_by(|a, b| b.cmp(a)); // Newest first

            let limit = param.limit.unwrap_or(10).clamp(1, 30) as usize;
            let selected: Vec<u32> = seq_vec.into_iter().take(limit).collect();
            let query_str = selected
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let messages = match session.fetch(&query_str, "(BODY.PEEK[HEADER.FIELDS (FROM SUBJECT DATE)])") {
                Ok(msgs) => msgs,
                Err(e) => return format!("{{\"error\": \"Failed to fetch headers: {e}\"}}"),
            };

            let mut email_list = Vec::new();
            for msg in &messages {
                let id = msg.message;
                let mut from = String::new();
                let mut subject = String::new();
                let mut date = String::new();

                if let Some(header_bytes) = msg.header() {
                    if let Ok((parsed, _)) = mailparse::parse_headers(header_bytes) {
                        for h in parsed {
                            match h.get_key().to_lowercase().as_str() {
                                "from" => from = h.get_value(),
                                "subject" => subject = h.get_value(),
                                "date" => date = h.get_value(),
                                _ => {}
                            }
                        }
                    }
                }

                email_list.push(serde_json::json!({
                    "id": id,
                    "from": from,
                    "subject": subject,
                    "date": date
                }));
            }

            let _ = session.logout();
            serde_json::to_string_pretty(&email_list).unwrap_or_else(|_| "[]".into())
        })
        .await
        .unwrap_or_else(|e| format!("{{\"error\": \"Task join error: {e}\"}}"))
    }

    #[tool(name = "gmail_read_email", description = "Read complete content and body of an email by sequence ID")]
    pub async fn read_email(&self, #[tool(aggr)] param: ReadEmailParam) -> String {
        tokio::task::spawn_blocking(move || {
            let mut session = match Self::connect_imap_sync() {
                Ok(s) => s,
                Err(e) => return format!("{{\"error\": \"{e}\"}}"),
            };

            if let Err(e) = session.select("INBOX") {
                return format!("{{\"error\": \"Failed to select INBOX: {e}\"}}");
            }

            let messages = match session.fetch(param.id.to_string(), "BODY[]") {
                Ok(m) => m,
                Err(e) => return format!("{{\"error\": \"Failed to fetch message {}: {e}\"}}", param.id),
            };

            let mut full_body = String::new();
            let mut subject = String::new();
            let mut from = String::new();
            let mut date = String::new();

            if let Some(msg) = messages.iter().next() {
                if let Some(body_bytes) = msg.body() {
                    if let Ok(mail) = mailparse::parse_mail(body_bytes) {
                        for h in &mail.headers {
                            match h.get_key().to_lowercase().as_str() {
                                "subject" => subject = h.get_value(),
                                "from" => from = h.get_value(),
                                "date" => date = h.get_value(),
                                _ => {}
                            }
                        }

                        if let Ok(content) = mail.get_body() {
                            full_body = content;
                        } else if !mail.subparts.is_empty() {
                            for sub in &mail.subparts {
                                if let Ok(c) = sub.get_body() {
                                    full_body.push_str(&c);
                                    full_body.push('\n');
                                }
                            }
                        }
                    }
                }
            }

            let _ = session.logout();

            serde_json::to_string_pretty(&serde_json::json!({
                "id": param.id,
                "from": from,
                "subject": subject,
                "date": date,
                "body": full_body.trim()
            }))
            .unwrap_or_else(|_| "{}".into())
        })
        .await
        .unwrap_or_else(|e| format!("{{\"error\": \"Task join error: {e}\"}}"))
    }

    #[tool(name = "gmail_send_email", description = "Send an email via Gmail SMTP")]
    pub async fn send_email(&self, #[tool(aggr)] param: SendEmailParam) -> String {
        let (my_email, pass) = match Self::get_credentials() {
            Ok(c) => c,
            Err(e) => return format!("{{\"error\": \"{e}\"}}"),
        };

        let from_header = match param.from_name {
            Some(ref name) => format!("{name} <{my_email}>"),
            None => format!("Muhammad Adriansyah <{my_email}>"),
        };

        let email = match Message::builder()
            .from(match from_header.parse() {
                Ok(f) => f,
                Err(e) => return format!("{{\"error\": \"Invalid from address: {e}\"}}"),
            })
            .to(match param.to.parse() {
                Ok(t) => t,
                Err(e) => return format!("{{\"error\": \"Invalid recipient address: {e}\"}}"),
            })
            .subject(&param.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(param.body)
        {
            Ok(m) => m,
            Err(e) => return format!("{{\"error\": \"Failed to build email: {e}\"}}"),
        };

        let creds = Credentials::new(my_email, pass);
        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")
                .map_err(|e| format!("{e}"))
                .unwrap()
                .credentials(creds)
                .build();

        match mailer.send(email).await {
            Ok(_) => serde_json::json!({
                "status": "success",
                "message": format!("Email successfully sent to {}", param.to),
                "subject": param.subject
            })
            .to_string(),
            Err(e) => format!("{{\"error\": \"Failed to send email: {e}\"}}"),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for GmailService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "smcp-gmail".into(),
                version: "0.2.0".into(),
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some("SMCP Gmail server for checking, reading, and sending emails.".into()),
            ..Default::default()
        }
    }
}
