use rmcp::{
    model::{Implementation, ServerCapabilities, ServerInfo},
    ServerHandler, tool,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

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

impl BitwardenService {
    fn resolve_master_password() -> Result<String, String> {
        // 1. Check environment variable
        if let Ok(val) = env::var("BW_PASSWORD") {
            if !val.trim().is_empty() {
                return Ok(val.trim().to_string());
            }
        }
        if let Ok(val) = env::var("BITWARDEN_MASTER_PASSWORD") {
            if !val.trim().is_empty() {
                return Ok(val.trim().to_string());
            }
        }

        // 2. Check candidate credential file paths (~/.config/credentials/...)
        let mut candidate_paths = Vec::new();
        if let Ok(home) = env::var("HOME") {
            let h = PathBuf::from(home);
            candidate_paths.push(h.join(".config/credentials/bitwarden_master_password"));
            candidate_paths.push(h.join(".config/credentials/bitwarden_credentials"));
            candidate_paths.push(h.join(".config/bitwarden/credentials"));
        }

        for path in candidate_paths {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(val) = trimmed.strip_prefix("BITWARDEN_MASTER_PASSWORD=") {
                            return Ok(val.trim().to_string());
                        }
                        if let Some(val) = trimmed.strip_prefix("BW_PASSWORD=") {
                            return Ok(val.trim().to_string());
                        }
                    }
                    let raw = content.trim();
                    if !raw.is_empty() && !raw.contains('=') {
                        return Ok(raw.to_string());
                    }
                }
            }
        }

        Err("Bitwarden master password not found. Please set BW_PASSWORD or BITWARDEN_MASTER_PASSWORD environment variable or create ~/.config/credentials/bitwarden_master_password.".into())
    }

    fn get_or_unlock_session(&self) -> Result<String, String> {
        let mut lock = self.session.lock().map_err(|e| e.to_string())?;
        if let Some(ref s) = *lock {
            return Ok(s.clone());
        }

        // If user already passed BW_SESSION in environment, reuse it
        if let Ok(session) = env::var("BW_SESSION") {
            if !session.trim().is_empty() {
                *lock = Some(session.trim().to_string());
                return Ok(session.trim().to_string());
            }
        }

        let password = Self::resolve_master_password()?;

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
    pub async fn status(&self) -> String {
        match self.run_bw(&["status"]) {
            Ok(res) => res,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(name = "bitwarden_sync", description = "Sync Bitwarden local cache with remote vault")]
    pub async fn sync(&self) -> String {
        match self.run_bw(&["sync"]) {
            Ok(_) => "{\"status\": \"synced\"}".into(),
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(name = "bitwarden_list_items", description = "List vault items. Optionally filter with query keyword.")]
    pub async fn list_items(&self, #[tool(aggr)] param: SearchParam) -> String {
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
    pub async fn get_item(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self.run_bw(&["get", "item", &param.id_or_name]) {
            Ok(res) => res,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(name = "bitwarden_get_password", description = "Get password directly for a specific vault item by name or ID")]
    pub async fn get_password(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self.run_bw(&["get", "password", &param.id_or_name]) {
            Ok(res) => res.trim().to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(name = "bitwarden_get_totp", description = "Generate/get current TOTP 2FA code for a specific vault item")]
    pub async fn get_totp(&self, #[tool(aggr)] param: GetItemParam) -> String {
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
                version: "0.2.0".into(),
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some("SMCP Bitwarden server for credential management.".into()),
            ..Default::default()
        }
    }
}
