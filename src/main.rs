mod servers;

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
    /// Bitwarden Vault Management
    #[command(subcommand)]
    Bitwarden(BitwardenCommands),

    /// Google Workspace Suite (Gmail, etc.)
    #[command(subcommand)]
    Workspace(WorkspaceCommands),
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
