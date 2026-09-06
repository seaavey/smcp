mod servers;
mod setup;

use clap::{Parser, Subcommand};
use rmcp::{transport::io::stdio, ServiceExt};
use servers::bitwarden::BitwardenService;
use servers::workspace::GmailService;

#[derive(Parser)]
#[command(name = "smcp", about = "Seaavey MCP Suite in Rust", version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive credential setup & verification wizards
    #[command(subcommand)]
    Setup(SetupCommands),

    /// Direct CLI call to a tool without starting MCP server
    #[command(subcommand)]
    Call(CallCommands),

    /// Bitwarden Vault Management
    #[command(subcommand)]
    Bitwarden(BitwardenCommands),

    /// Google Workspace Suite (Gmail, etc.)
    #[command(subcommand)]
    Workspace(WorkspaceCommands),
}

#[derive(Subcommand)]
enum SetupCommands {
    /// Setup Gmail credentials (App Password)
    Gmail,
    /// Setup Bitwarden Master Password
    Bitwarden,
}

#[derive(Subcommand)]
enum CallCommands {
    /// Check emails (Gmail)
    GmailCheck {
        #[arg(short, long, default_value = "10")]
        limit: u32,
        #[arg(short, long, default_value = "unread")]
        filter: String,
    },
    /// Read an email by sequence ID
    GmailRead { id: u32 },
    /// Send an email
    GmailSend {
        #[arg(short, long)]
        to: String,
        #[arg(short, long)]
        subject: String,
        #[arg(short, long)]
        body: String,
        #[arg(long)]
        from_name: Option<String>,
    },
    /// List Bitwarden items
    BwList {
        #[arg(short, long)]
        query: Option<String>,
    },
    /// Get Bitwarden password
    BwPassword { id_or_name: String },
    /// Get Bitwarden TOTP
    BwTotp { id_or_name: String },
}

#[derive(Subcommand)]
enum BitwardenCommands {
    /// Run Bitwarden MCP server (stdio)
    Serve,
}

#[derive(Subcommand)]
enum WorkspaceCommands {
    /// Run Gmail MCP server (stdio)
    Gmail,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup(SetupCommands::Gmail) => {
            setup::setup_gmail()?;
        }
        Commands::Setup(SetupCommands::Bitwarden) => {
            setup::setup_bitwarden()?;
        }
        Commands::Call(CallCommands::GmailCheck { limit, filter }) => {
            let svc = GmailService::default();
            let res = svc
                .check_emails(servers::workspace::gmail::CheckEmailsParam {
                    limit: Some(limit),
                    filter: Some(filter),
                })
                .await;
            println!("{res}");
        }
        Commands::Call(CallCommands::GmailRead { id }) => {
            let svc = GmailService::default();
            let res = svc
                .read_email(servers::workspace::gmail::ReadEmailParam { id })
                .await;
            println!("{res}");
        }
        Commands::Call(CallCommands::GmailSend {
            to,
            subject,
            body,
            from_name,
        }) => {
            let svc = GmailService::default();
            let res = svc
                .send_email(servers::workspace::gmail::SendEmailParam {
                    to,
                    subject,
                    body,
                    from_name,
                })
                .await;
            println!("{res}");
        }
        Commands::Call(CallCommands::BwList { query }) => {
            let svc = BitwardenService::default();
            let res = svc
                .list_items(servers::bitwarden::SearchParam { query })
                .await;
            println!("{res}");
        }
        Commands::Call(CallCommands::BwPassword { id_or_name }) => {
            let svc = BitwardenService::default();
            let res = svc
                .get_password(servers::bitwarden::GetItemParam { id_or_name })
                .await;
            println!("{res}");
        }
        Commands::Call(CallCommands::BwTotp { id_or_name }) => {
            let svc = BitwardenService::default();
            let res = svc
                .get_totp(servers::bitwarden::GetItemParam { id_or_name })
                .await;
            println!("{res}");
        }
        Commands::Bitwarden(BitwardenCommands::Serve) => {
            let server = BitwardenService::default();
            let transport = stdio();
            let running = server.serve(transport).await?;
            running.waiting().await?;
        }
        Commands::Workspace(WorkspaceCommands::Gmail) => {
            let server = GmailService::default();
            let transport = stdio();
            let running = server.serve(transport).await?;
            running.waiting().await?;
        }
    }

    Ok(())
}
