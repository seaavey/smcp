use lettre::{
    message::{header::ContentType, Attachment, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use rmcp::{
    model::{Implementation, ServerCapabilities, ServerInfo},
    ServerHandler, tool,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct GmailService;

#[derive(Deserialize, JsonSchema)]
pub struct SetCredentialsParam {
    #[schemars(description = "User Gmail address (e.g. user@gmail.com)")]
    pub email: String,
    #[schemars(description = "16-character Google App Password (generated from https://myaccount.google.com/apppasswords)")]
    pub app_password: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CheckEmailsParam {
    #[schemars(description = "Maximum number of recent emails to retrieve (default: 10, max: 30)")]
    pub limit: Option<u32>,
    #[schemars(description = "Filter type: 'unread', 'all', 'read', 'starred' (default: 'unread')")]
    pub filter: Option<String>,
    #[schemars(description = "Mailbox/folder: 'inbox', 'spam', 'trash', 'sent', 'drafts', 'all', 'starred', 'important' (default: 'inbox')")]
    pub folder: Option<String>,
    #[schemars(description = "Optional search query to filter by sender, subject, or keyword")]
    pub query: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadEmailParam {
    #[schemars(description = "The permanent UID of the email from check_emails")]
    pub uid: u32,
    #[schemars(description = "Mailbox/folder the email is in (default: 'inbox')")]
    pub folder: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReplyEmailParam {
    #[schemars(description = "The permanent UID of the email to reply to")]
    pub uid: u32,
    #[schemars(description = "Mailbox/folder where original email resides (default: 'inbox')")]
    pub folder: Option<String>,
    #[schemars(description = "Reply body text")]
    pub body: String,
    #[schemars(description = "Optional custom sender display name")]
    pub from_name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ManageEmailParam {
    #[schemars(description = "The permanent UID of the email")]
    pub uid: u32,
    #[schemars(description = "Action to execute: 'mark_read', 'mark_unread', 'star', 'unstar', 'trash'")]
    pub action: String,
    #[schemars(description = "Mailbox/folder the email is currently in (default: 'inbox')")]
    pub folder: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SendEmailParam {
    #[schemars(description = "Recipient email address")]
    pub to: String,
    #[schemars(description = "Email subject")]
    pub subject: String,
    #[schemars(description = "Email body text (plain text or HTML)")]
    pub body: String,
    #[schemars(description = "True if body is formatted HTML (default: false)")]
    pub is_html: Option<bool>,
    #[schemars(description = "Optional custom sender display name")]
    pub from_name: Option<String>,
    #[schemars(description = "Optional local file paths to attach")]
    pub attachments: Option<Vec<String>>,
}

impl GmailService {
    fn resolve_credentials() -> Result<(String, String), String> {
        let mut email = env::var("GMAIL_EMAIL").unwrap_or_default().trim().to_string();
        let mut app_password = env::var("GMAIL_APP_PASSWORD").unwrap_or_default().trim().to_string();

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

        let mut session = client
            .login(email, pass)
            .map_err(|(e, _)| format!("IMAP authentication failed: {e}. Check your App Password."))?;

        let _ = session.logout();
        Ok(())
    }

    fn connect_imap_sync() -> Result<imap::Session<native_tls::TlsStream<std::net::TcpStream>>, String> {
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

    fn detect_sender_name(session: &mut imap::Session<native_tls::TlsStream<std::net::TcpStream>>, my_email: &str) -> Option<String> {
        let sent_folders = ["[Gmail]/Sent Mail", "INBOX"];
        for folder in sent_folders {
            if session.select(folder).is_ok() {
                if let Ok(seqs) = session.search(format!("FROM \"{my_email}\"")) {
                    if let Some(&last_id) = seqs.iter().max() {
                        if let Ok(msgs) = session.fetch(last_id.to_string(), "(BODY.PEEK[HEADER.FIELDS (FROM)])") {
                            for m in &msgs {
                                if let Some(hdr) = m.header() {
                                    if let Ok((headers, _)) = mailparse::parse_headers(hdr) {
                                        for h in headers {
                                            if h.get_key().eq_ignore_ascii_case("from") {
                                                let val = h.get_value();
                                                if let Some(idx) = val.find('<') {
                                                    let name = val[..idx].trim().trim_matches('"').trim();
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

#[tool(tool_box)]
impl GmailService {
    #[tool(
        name = "gmail_set_credentials",
        description = "Set and save Gmail credentials (email and App Password) directly via MCP, testing the connection immediately"
    )]
    pub async fn set_credentials(&self, #[tool(aggr)] param: SetCredentialsParam) -> String {
        let email = param.email.trim().to_string();
        let clean_pwd = param.app_password.replace(' ', "").trim().to_string();

        if email.is_empty() || !email.contains('@') {
            return serde_json::json!({
                "status": "error",
                "message": "Invalid email address."
            }).to_string();
        }

        if clean_pwd.is_empty() {
            return serde_json::json!({
                "status": "error",
                "message": "App Password cannot be empty."
            }).to_string();
        }

        let em = email.clone();
        let pw = clean_pwd.clone();
        let test_res = tokio::task::spawn_blocking(move || {
            Self::test_imap_login(&em, &pw)
        }).await.unwrap_or_else(|e| Err(e.to_string()));

        if let Err(err) = test_res {
            return serde_json::json!({
                "status": "error",
                "message": format!("Verification failed: {err}")
            }).to_string();
        }

        let base = if let Ok(home) = env::var("HOME") {
            PathBuf::from(home)
        } else {
            PathBuf::from(".")
        };
        let creds_dir = base.join(".config").join("credentials");
        if let Err(e) = fs::create_dir_all(&creds_dir) {
            return serde_json::json!({
                "status": "error",
                "message": format!("Failed to create credentials directory: {e}")
            }).to_string();
        }

        let creds_file = creds_dir.join("workspace-google");
        let content = format!("GMAIL_EMAIL={email}\nGMAIL_APP_PASSWORD={clean_pwd}\n");
        if let Err(e) = fs::write(&creds_file, content) {
            return serde_json::json!({
                "status": "error",
                "message": format!("Failed to write credentials file: {e}")
            }).to_string();
        }

        serde_json::json!({
            "status": "success",
            "message": format!("Gmail credentials successfully verified and saved for {email}!"),
            "path": creds_file.display().to_string()
        }).to_string()
    }

    #[tool(
        name = "gmail_check_emails",
        description = "Check recent emails across mailboxes (INBOX, Spam, Trash, Sent, Starred) with permanent UIDs"
    )]
    pub async fn check_emails(&self, #[tool(aggr)] param: CheckEmailsParam) -> String {
        tokio::task::spawn_blocking(move || {
            let mut session = match Self::connect_imap_sync() {
                Ok(s) => s,
                Err(e) => return format!("{{\"error\": \"{e}\"}}"),
            };

            let mailbox = Self::resolve_mailbox_name(param.folder.as_deref());
            if let Err(e) = session.select(mailbox) {
                return format!("{{\"error\": \"Failed to select mailbox '{mailbox}': {e}\"}}");
            }

            let filter = param.filter.unwrap_or_else(|| "unread".into());
            let base_query = match filter.to_lowercase().as_str() {
                "all" => "ALL",
                "read" | "seen" => "SEEN",
                "starred" | "flagged" => "FLAGGED",
                _ => "UNSEEN",
            };

            let imap_search_query = if let Some(ref q) = param.query {
                let trimmed = q.trim();
                if !trimmed.is_empty() {
                    format!("{base_query} TEXT \"{trimmed}\"")
                } else {
                    base_query.to_string()
                }
            } else {
                base_query.to_string()
            };

            let uid_set = match session.uid_search(&imap_search_query) {
                Ok(set) => set,
                Err(e) => return format!("{{\"error\": \"Failed to search emails with query '{imap_search_query}': {e}\"}}"),
            };

            if uid_set.is_empty() {
                let _ = session.logout();
                return serde_json::json!({
                    "mailbox": mailbox,
                    "message": format!("No emails found matching query: {imap_search_query}"),
                    "emails": []
                }).to_string();
            }

            let mut uid_vec: Vec<u32> = uid_set.into_iter().collect();
            uid_vec.sort_by(|a, b| b.cmp(a)); // Newest first

            let limit = param.limit.unwrap_or(10).clamp(1, 30) as usize;
            let selected: Vec<u32> = uid_vec.into_iter().take(limit).collect();
            let query_str = selected
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let messages = match session.uid_fetch(&query_str, "(BODY.PEEK[HEADER.FIELDS (FROM SUBJECT DATE)] FLAGS UID)") {
                Ok(msgs) => msgs,
                Err(e) => return format!("{{\"error\": \"Failed to fetch headers: {e}\"}}"),
            };

            let mut email_list = Vec::new();
            for msg in &messages {
                let uid = msg.uid.unwrap_or(0);
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
                    "uid": uid,
                    "mailbox": mailbox,
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

    #[tool(name = "gmail_read_email", description = "Read complete content, attachments metadata, and body of an email by permanent UID")]
    pub async fn read_email(&self, #[tool(aggr)] param: ReadEmailParam) -> String {
        tokio::task::spawn_blocking(move || {
            let mut session = match Self::connect_imap_sync() {
                Ok(s) => s,
                Err(e) => return format!("{{\"error\": \"{e}\"}}"),
            };

            let mailbox = Self::resolve_mailbox_name(param.folder.as_deref());
            if let Err(e) = session.select(mailbox) {
                return format!("{{\"error\": \"Failed to select mailbox '{mailbox}': {e}\"}}");
            }

            let messages = match session.uid_fetch(param.uid.to_string(), "BODY[]") {
                Ok(m) => m,
                Err(e) => return format!("{{\"error\": \"Failed to fetch message UID {} from '{}': {e}\"}}", param.uid, mailbox),
            };

            let mut full_body = String::new();
            let mut subject = String::new();
            let mut from = String::new();
            let mut to = String::new();
            let mut date = String::new();
            let mut message_id = String::new();
            let mut attachments = Vec::new();

            if let Some(msg) = messages.iter().next() {
                if let Some(body_bytes) = msg.body() {
                    if let Ok(mail) = mailparse::parse_mail(body_bytes) {
                        for h in &mail.headers {
                            match h.get_key().to_lowercase().as_str() {
                                "subject" => subject = h.get_value(),
                                "from" => from = h.get_value(),
                                "to" => to = h.get_value(),
                                "date" => date = h.get_value(),
                                "message-id" => message_id = h.get_value(),
                                _ => {}
                            }
                        }

                        if mail.subparts.is_empty() {
                            if let Ok(content) = mail.get_body() {
                                full_body = content;
                            }
                        } else {
                            for sub in &mail.subparts {
                                let ctype = sub.ctype.mimetype.to_lowercase();
                                let is_attachment = sub.get_content_disposition().disposition == mailparse::DispositionType::Attachment
                                    || sub.get_content_disposition().params.contains_key("filename");

                                if is_attachment {
                                    let filename = sub.get_content_disposition().params.get("filename")
                                        .cloned()
                                        .unwrap_or_else(|| "attachment".into());
                                    attachments.push(serde_json::json!({
                                        "filename": filename,
                                        "content_type": ctype,
                                        "size_approx": sub.get_body_raw().map(|b| b.len()).unwrap_or(0)
                                    }));
                                } else if ctype.contains("text/plain") || ctype.contains("text/html") {
                                    if let Ok(c) = sub.get_body() {
                                        if !c.trim().is_empty() {
                                            full_body.push_str(&c);
                                            full_body.push('\n');
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let _ = session.logout();

            serde_json::to_string_pretty(&serde_json::json!({
                "uid": param.uid,
                "mailbox": mailbox,
                "from": from,
                "to": to,
                "subject": subject,
                "date": date,
                "message_id": message_id,
                "attachments": attachments,
                "body": full_body.trim()
            }))
            .unwrap_or_else(|_| "{}".into())
        })
        .await
        .unwrap_or_else(|e| format!("{{\"error\": \"Task join error: {e}\"}}"))
    }

    #[tool(name = "gmail_manage_email", description = "Manage an email: mark read/unread, star/unstar, or move to trash by permanent UID")]
    pub async fn manage_email(&self, #[tool(aggr)] param: ManageEmailParam) -> String {
        tokio::task::spawn_blocking(move || {
            let mut session = match Self::connect_imap_sync() {
                Ok(s) => s,
                Err(e) => return format!("{{\"error\": \"{e}\"}}"),
            };

            let mailbox = Self::resolve_mailbox_name(param.folder.as_deref());
            if let Err(e) = session.select(mailbox) {
                return format!("{{\"error\": \"Failed to select mailbox '{mailbox}': {e}\"}}");
            }

            let uid_str = param.uid.to_string();
            let action_norm = param.action.to_lowercase();

            let res = match action_norm.as_str() {
                "mark_read" | "read" => session.uid_store(&uid_str, "+FLAGS (\\Seen)"),
                "mark_unread" | "unread" => session.uid_store(&uid_str, "-FLAGS (\\Seen)"),
                "star" => session.uid_store(&uid_str, "+FLAGS (\\Flagged)"),
                "unstar" => session.uid_store(&uid_str, "-FLAGS (\\Flagged)"),
                "trash" | "delete" => {
                    // Copy to Trash and flag Deleted in current mailbox
                    let _ = session.uid_copy(&uid_str, "[Gmail]/Trash");
                    session.uid_store(&uid_str, "+FLAGS (\\Deleted)")
                }
                _ => return serde_json::json!({
                    "status": "error",
                    "message": format!("Unknown action '{}'. Valid: mark_read, mark_unread, star, unstar, trash", param.action)
                }).to_string(),
            };

            let _ = session.logout();

            match res {
                Ok(_) => serde_json::json!({
                    "status": "success",
                    "action": action_norm,
                    "uid": param.uid,
                    "mailbox": mailbox
                }).to_string(),
                Err(e) => format!("{{\"error\": \"Failed to execute action: {e}\"}}"),
            }
        })
        .await
        .unwrap_or_else(|e| format!("{{\"error\": \"Task join error: {e}\"}}"))
    }

    #[tool(name = "gmail_reply_email", description = "Reply to an existing email thread using UID (automatically sets In-Reply-To, References, and Re: Subject)")]
    pub async fn reply_email(&self, #[tool(aggr)] param: ReplyEmailParam) -> String {
        let (my_email, pass) = match Self::resolve_credentials() {
            Ok(c) => c,
            Err(e) => return format!("{{\"error\": \"{e}\"}}"),
        };

        let mailbox = Self::resolve_mailbox_name(param.folder.as_deref());
        let uid = param.uid;

        let orig_meta = tokio::task::spawn_blocking(move || {
            let mut session = match Self::connect_imap_sync() {
                Ok(s) => s,
                Err(e) => return Err(e),
            };
            if let Err(e) = session.select(mailbox) {
                return Err(format!("Failed to select mailbox: {e}"));
            }
            let msgs = session.uid_fetch(uid.to_string(), "(BODY.PEEK[HEADER.FIELDS (FROM SUBJECT MESSAGE-ID REFERENCES)])")
                .map_err(|e| format!("Fetch failed: {e}"))?;

            let mut from = String::new();
            let mut subject = String::new();
            let mut message_id = String::new();
            let mut references = String::new();

            if let Some(m) = msgs.first() {
                if let Some(hdr) = m.header() {
                    if let Ok((parsed, _)) = mailparse::parse_headers(hdr) {
                        for h in parsed {
                            match h.get_key().to_lowercase().as_str() {
                                "from" => from = h.get_value(),
                                "subject" => subject = h.get_value(),
                                "message-id" => message_id = h.get_value(),
                                "references" => references = h.get_value(),
                                _ => {}
                            }
                        }
                    }
                }
            }
            let _ = session.logout();
            Ok((from, subject, message_id, references))
        }).await.unwrap_or_else(|e| Err(e.to_string()));

        let (orig_from, orig_subject, orig_msg_id, orig_refs) = match orig_meta {
            Ok(meta) => meta,
            Err(e) => return format!("{{\"error\": \"Could not fetch original email: {e}\"}}"),
        };

        if orig_from.is_empty() {
            return format!("{{\"error\": \"Original sender address not found for UID {uid}\"}}");
        }

        // Reply To address extraction
        let reply_to = if let Some(idx) = orig_from.find('<') {
            orig_from[idx+1..].trim_end_matches('>').trim().to_string()
        } else {
            orig_from.trim().to_string()
        };

        let reply_subject = if orig_subject.to_lowercase().starts_with("re:") {
            orig_subject
        } else {
            format!("Re: {orig_subject}")
        };

        let email_for_task = my_email.clone();
        let detected_name = if param.from_name.is_none() {
            tokio::task::spawn_blocking(move || {
                if let Ok(mut session) = Self::connect_imap_sync() {
                    let res = Self::detect_sender_name(&mut session, &email_for_task);
                    let _ = session.logout();
                    res
                } else {
                    None
                }
            })
            .await
            .unwrap_or(None)
        } else {
            None
        };

        let from_header = match param.from_name.or(detected_name) {
            Some(name) => format!("{name} <{my_email}>"),
            None => my_email.clone(),
        };

        let mut builder = Message::builder()
            .from(match from_header.parse() {
                Ok(f) => f,
                Err(e) => return format!("{{\"error\": \"Invalid from address: {e}\"}}"),
            })
            .to(match reply_to.parse() {
                Ok(t) => t,
                Err(e) => return format!("{{\"error\": \"Invalid recipient address '{reply_to}': {e}\"}}"),
            })
            .subject(&reply_subject)
            .header(ContentType::TEXT_PLAIN);

        if !orig_msg_id.is_empty() {
            builder = builder.header(lettre::message::header::InReplyTo::from(orig_msg_id.clone()));
            let new_refs = if orig_refs.is_empty() {
                orig_msg_id
            } else {
                format!("{orig_refs} {orig_msg_id}")
            };
            builder = builder.header(lettre::message::header::References::from(new_refs));
        }

        let email = match builder.body(param.body) {
            Ok(m) => m,
            Err(e) => return format!("{{\"error\": \"Failed to build reply email: {e}\"}}"),
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
                "action": "reply",
                "in_reply_to_uid": uid,
                "reply_to": reply_to,
                "subject": reply_subject
            })
            .to_string(),
            Err(e) => format!("{{\"error\": \"Failed to send reply email: {e}\"}}"),
        }
    }

    #[tool(name = "gmail_send_email", description = "Send an email via Gmail SMTP (supports text or HTML)")]
    pub async fn send_email(&self, #[tool(aggr)] param: SendEmailParam) -> String {
        let (my_email, pass) = match Self::resolve_credentials() {
            Ok(c) => c,
            Err(e) => return format!("{{\"error\": \"{e}\"}}"),
        };

        let email_for_task = my_email.clone();
        let detected_name = if param.from_name.is_none() {
            tokio::task::spawn_blocking(move || {
                if let Ok(mut session) = Self::connect_imap_sync() {
                    let res = Self::detect_sender_name(&mut session, &email_for_task);
                    let _ = session.logout();
                    res
                } else {
                    None
                }
            })
            .await
            .unwrap_or(None)
        } else {
            None
        };

        let from_header = match param.from_name.or(detected_name) {
            Some(name) => format!("{name} <{my_email}>"),
            None => my_email.clone(),
        };

        let is_html = param.is_html.unwrap_or(false);
        let ctype = if is_html {
            ContentType::TEXT_HTML
        } else {
            ContentType::TEXT_PLAIN
        };

        let body_part = SinglePart::builder().header(ctype).body(param.body);
        let mut multipart = MultiPart::mixed().singlepart(body_part);
        for attachment_path in param.attachments.unwrap_or_default() {
            let path = PathBuf::from(&attachment_path);
            let filename = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) if !name.is_empty() => name.to_string(),
                _ => return format!("{{\"error\": \"Invalid attachment path: {attachment_path}\"}}"),
            };
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(e) => return format!("{{\"error\": \"Failed to read attachment '{attachment_path}': {e}\"}}"),
            };
            let content_type = match ContentType::parse(mime_type_for_path(&path)) {
                Ok(content_type) => content_type,
                Err(e) => return format!("{{\"error\": \"Invalid attachment MIME type for '{attachment_path}': {e}\"}}"),
            };
            multipart = multipart.singlepart(Attachment::new(filename).body(bytes, content_type));
        }

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
            .multipart(multipart)
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
                "from": from_header,
                "subject": param.subject
            })
            .to_string(),
            Err(e) => format!("{{\"error\": \"Failed to send email: {e}\"}}"),
        }
    }
}

fn mime_type_for_path(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
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
            instructions: Some("SMCP Gmail server for checking, reading, managing, and sending emails.".into()),
            ..Default::default()
        }
    }
}
