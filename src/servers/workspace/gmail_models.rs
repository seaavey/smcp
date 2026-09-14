use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct SetCredentialsParam {
    #[schemars(description = "User Gmail address (e.g. user@gmail.com)")]
    pub email: String,
    #[schemars(
        description = "16-character Google App Password (generated from https://myaccount.google.com/apppasswords)"
    )]
    pub app_password: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CheckEmailsParam {
    #[schemars(description = "Maximum number of recent emails to retrieve (default: 10, max: 30)")]
    pub limit: Option<u32>,
    #[schemars(
        description = "Filter type: 'unread', 'all', 'read', 'starred' (default: 'unread')"
    )]
    pub filter: Option<String>,
    #[schemars(
        description = "Mailbox/folder: 'inbox', 'spam', 'trash', 'sent', 'drafts', 'all', 'starred', 'important' (default: 'inbox')"
    )]
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
    #[schemars(
        description = "Action to execute: 'mark_read', 'mark_unread', 'star', 'unstar', 'trash'"
    )]
    pub action: String,
    #[schemars(description = "Mailbox/folder the email is currently in (default: 'inbox')")]
    pub folder: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct BulkManageEmailParam {
    #[schemars(
        description = "Action to execute: 'mark_read', 'mark_unread', 'star', 'unstar', or 'trash'"
    )]
    pub action: String,
    #[schemars(description = "Mailbox/folder to search (default: 'inbox')")]
    pub folder: Option<String>,
    #[schemars(
        description = "Filter type: 'unread', 'all', 'read', or 'starred' (default: 'unread')"
    )]
    pub filter: Option<String>,
    #[schemars(description = "Optional search text matched against message content")]
    pub query: Option<String>,
    #[schemars(description = "Maximum number of messages to affect (default: 30, max: 100)")]
    pub limit: Option<u32>,
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

#[derive(Deserialize, JsonSchema)]
pub struct BulkSendEmailParam {
    #[schemars(description = "Recipient email addresses; maximum 100")]
    pub to: Vec<String>,
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
