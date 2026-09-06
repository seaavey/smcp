use clap::{Parser, Subcommand};
use rmcp::{
    model::{Implementation, ServerCapabilities, ServerInfo},
    transport::io::stdio,
    ServerHandler, ServiceExt, tool,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "smcp", about = "Seaavey MCP Suite in Rust", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run Bitwarden MCP Server (stdio transport)
    Bitwarden,
}

#[derive(Debug, Clone, Default)]
pub struct BitwardenService {
    session: Arc<Mutex<Option<String>>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchParam {
    #[schemars(description = "Keyword to filter vault items by name")]
    pub query: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct GetItemParam {
    #[schemars(description = "Exact name or ID of the vault item")]
    pub id_or_name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateItemParam {
    #[schemars(description = "Name of the vault item")]
    pub name: String,
    #[schemars(description = "Username or email")]
    pub username: String,
    #[schemars(description = "Password")]
    pub password: String,
    #[schemars(description = "Optional login URL/URI")]
    pub uri: Option<String>,
}

impl BitwardenService {
    fn get_or_unlock_session(&self) -> Result<String, String> {
        let mut lock = self.session.lock().map_err(|e| e.to_string())?;
        if let Some(ref s) = *lock {
            return Ok(s.clone());
        }

        let master_pwd_path = "/root/.config/credentials/bitwarden_master_password";
        let content = fs::read_to_string(master_pwd_path)
            .map_err(|e| format!("Failed to read master password file: {e}"))?;

        let mut password = String::new();
        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("BITWARDEN_MASTER_PASSWORD=") {
                password = val.trim().to_string();
                break;
            }
        }
        if password.is_empty() {
            password = content.trim().to_string();
        }

        let output = Command::new("bw")
            .args(["unlock", &password, "--raw"])
            .output()
            .map_err(|e| format!("Failed to execute bw unlock: {e}"))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("bw unlock failed: {err}"));
        }

        let session = String::from_utf8_lossy(&output.stdout).trim().to_string();
        *lock = Some(session.clone());
        Ok(session)
    }

    fn run_bw(&self, args: &[&str]) -> Result<String, String> {
        let session = self.get_or_unlock_session()?;
        let mut cmd_args = args.to_vec();
        cmd_args.push("--session");
        cmd_args.push(&session);

        let output = Command::new("bw")
            .args(&cmd_args)
            .output()
            .map_err(|e| format!("Failed to run bw: {e}"))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("bw error: {err}"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[tool(tool_box)]
impl BitwardenService {
    #[tool(name = "bitwarden_status", description = "Check Bitwarden status and sync status")]
    async fn status(&self) -> String {
        match self.run_bw(&["status"]) {
            Ok(res) => res,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(name = "bitwarden_sync", description = "Sync Bitwarden local cache with remote vault")]
    async fn sync(&self) -> String {
        match self.run_bw(&["sync"]) {
            Ok(_) => "{\"status\": \"synced\"}".into(),
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(name = "bitwarden_list_items", description = "List vault items. Optionally filter with query keyword.")]
    async fn list_items(&self, #[tool(aggr)] param: SearchParam) -> String {
        let mut args = vec!["list", "items"];
        let q = param.query.unwrap_or_default();
        if !q.is_empty() {
            args.push("--search");
            args.push(&q);
        }

        match self.run_bw(&args) {
            Ok(raw) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(arr) = json.as_array() {
                        let simplified: Vec<serde_json::Value> = arr
                            .iter()
                            .map(|item| {
                                serde_json::json!({
                                    "id": item.get("id"),
                                    "name": item.get("name"),
                                    "type": item.get("type"),
                                    "username": item.get("login").and_then(|l| l.get("username")),
                                })
                            })
                            .collect();
                        return serde_json::to_string_pretty(&simplified).unwrap_or(raw);
                    }
                }
                raw
            }
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(name = "bitwarden_get_item", description = "Get details of a specific vault item by name or ID")]
    async fn get_item(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self.run_bw(&["get", "item", &param.id_or_name]) {
            Ok(res) => res,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(name = "bitwarden_get_password", description = "Get password directly for a specific vault item by name or ID")]
    async fn get_password(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self.run_bw(&["get", "password", &param.id_or_name]) {
            Ok(res) => res.trim().to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(name = "bitwarden_get_totp", description = "Generate/get current TOTP 2FA code for a specific vault item")]
    async fn get_totp(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self.run_bw(&["get", "totp", &param.id_or_name]) {
            Ok(res) => res.trim().to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for BitwardenService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "smcp-bitwarden".into(),
                version: "0.1.0".into(),
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some("Official SMCP Bitwarden server for credential management.".into()),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Bitwarden => {
            let server = BitwardenService::default();
            let transport = stdio();
            let running = server.serve(transport).await?;
            running.waiting().await?;
        }
    }

    Ok(())
}
