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
pub struct SetMasterPasswordParam {
    #[schemars(description = "Bitwarden Master Password to store and unlock vault with")]
    pub master_password: String,
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
pub struct CreateLoginItemParam {
    #[schemars(description = "Item name (e.g. 'GitHub Prod', 'OpenAI')")]
    pub name: String,
    #[schemars(description = "Username or email")]
    pub username: String,
    #[schemars(description = "Password for the login item")]
    pub password: String,
    #[schemars(description = "Optional website URI / URL")]
    pub uri: Option<String>,
    #[schemars(description = "Optional notes")]
    pub notes: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct GeneratePasswordParam {
    #[schemars(description = "Password length (default: 24)")]
    pub length: Option<u32>,
    #[schemars(description = "Include special characters (default: true)")]
    pub special: Option<bool>,
    #[schemars(description = "Include numbers (default: true)")]
    pub numbers: Option<bool>,
    #[schemars(description = "Include uppercase letters (default: true)")]
    pub uppercase: Option<bool>,
    #[schemars(description = "Include lowercase letters (default: true)")]
    pub lowercase: Option<bool>,
}

impl BitwardenService {
    fn resolve_master_password() -> Result<String, String> {
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

        if let Ok(home) = env::var("HOME") {
            let path = PathBuf::from(home).join(".config/credentials/bitwarden_master_password");
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(val) = trimmed.strip_prefix("BITWARDEN_MASTER_PASSWORD=") {
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

        Err("Bitwarden master password not configured. Please use tool `bitwarden_set_password` or run `smcp setup bitwarden`.".into())
    }

    fn get_or_unlock_session(&self) -> Result<String, String> {
        let mut lock = self.session.lock().map_err(|e| e.to_string())?;
        if let Some(ref s) = *lock {
            return Ok(s.clone());
        }

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

    fn run_bw_stdin(&self, args: &[&str], input: &str) -> Result<String, String> {
        use std::io::Write;
        let session = self.get_or_unlock_session()?;
        let mut cmd_args = args.to_vec();
        cmd_args.push("--session");
        cmd_args.push(&session);

        let mut child = Command::new("bw")
            .args(&cmd_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn bw: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).map_err(|e| format!("Write stdin failed: {e}"))?;
        }

        let output = child.wait_with_output().map_err(|e| format!("Wait bw failed: {e}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("bw error: {err}"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[tool(tool_box)]
impl BitwardenService {
    #[tool(
        name = "bitwarden_set_password",
        description = "Set and save Bitwarden Master Password directly via MCP, testing unlock immediately"
    )]
    pub async fn set_password(&self, #[tool(aggr)] param: SetMasterPasswordParam) -> String {
        let pwd = param.master_password.trim().to_string();
        if pwd.is_empty() {
            return serde_json::json!({
                "status": "error",
                "message": "Master password cannot be empty."
            }).to_string();
        }

        let output = match Command::new("bw").args(["unlock", &pwd, "--raw"]).output() {
            Ok(o) => o,
            Err(e) => return serde_json::json!({
                "status": "error",
                "message": format!("Failed to execute bw CLI: {e}")
            }).to_string(),
        };

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return serde_json::json!({
                "status": "error",
                "message": format!("Bitwarden unlock verification failed: {err}")
            }).to_string();
        }

        let new_session = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(mut lock) = self.session.lock() {
            *lock = Some(new_session);
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

        let creds_file = creds_dir.join("bitwarden_master_password");
        let content = format!("BITWARDEN_MASTER_PASSWORD={pwd}\n");
        if let Err(e) = fs::write(&creds_file, content) {
            return serde_json::json!({
                "status": "error",
                "message": format!("Failed to write credentials file: {e}")
            }).to_string();
        }

        serde_json::json!({
            "status": "success",
            "message": "Bitwarden master password verified, session unlocked, and saved!",
            "path": creds_file.display().to_string()
        }).to_string()
    }

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

    #[tool(name = "bitwarden_generate_password", description = "Generate a secure random password using Bitwarden CLI")]
    pub async fn generate_password(&self, #[tool(aggr)] param: GeneratePasswordParam) -> String {
        let length_str = param.length.unwrap_or(24).to_string();
        let mut args = vec!["generate", "--length", &length_str];

        let special = param.special.unwrap_or(true);
        let numbers = param.numbers.unwrap_or(true);
        let uppercase = param.uppercase.unwrap_or(true);
        let lowercase = param.lowercase.unwrap_or(true);

        if special {
            args.push("-s");
        }
        if uppercase {
            args.push("-u");
        }
        if lowercase {
            args.push("-l");
        }
        if numbers {
            args.push("-n");
        }

        let output = match Command::new("bw").args(&args).output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Ok(o) => format!("Error: {}", String::from_utf8_lossy(&o.stderr)),
            Err(e) => format!("Error running bw: {e}"),
        };

        serde_json::json!({
            "generated_password": output
        }).to_string()
    }

    #[tool(name = "bitwarden_create_login_item", description = "Create and save a new login item/credential into Bitwarden vault")]
    pub async fn create_login_item(&self, #[tool(aggr)] param: CreateLoginItemParam) -> String {
        // Step 1: get template
        let template_raw = match self.run_bw(&["get", "template", "item"]) {
            Ok(r) => r,
            Err(e) => return format!("{{\"error\": \"Failed to get item template: {e}\"}}"),
        };

        let login_template_raw = match self.run_bw(&["get", "template", "item.login"]) {
            Ok(r) => r,
            Err(e) => return format!("{{\"error\": \"Failed to get login template: {e}\"}}"),
        };

        let mut item: serde_json::Value = match serde_json::from_str(&template_raw) {
            Ok(v) => v,
            Err(e) => return format!("{{\"error\": \"Template parse error: {e}\"}}"),
        };

        let mut login: serde_json::Value = match serde_json::from_str(&login_template_raw) {
            Ok(v) => v,
            Err(e) => return format!("{{\"error\": \"Login template parse error: {e}\"}}"),
        };

        login["username"] = serde_json::Value::String(param.username);
        login["password"] = serde_json::Value::String(param.password);
        if let Some(uri) = param.uri {
            let mut uri_obj = serde_json::Map::new();
            uri_obj.insert("uri".into(), serde_json::Value::String(uri));
            login["uris"] = serde_json::Value::Array(vec![serde_json::Value::Object(uri_obj)]);
        }

        item["type"] = serde_json::json!(1); // 1 = Login
        item["name"] = serde_json::Value::String(param.name);
        if let Some(n) = param.notes {
            item["notes"] = serde_json::Value::String(n);
        }
        item["login"] = login;

        let payload_str = serde_json::to_string(&item).unwrap_or_default();

        // Step 2: encode via bw encode
        let encoded_output = match Command::new("bw")
            .arg("encode")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(payload_str.as_bytes());
                }
                match child.wait_with_output() {
                    Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
                    Ok(out) => return format!("{{\"error\": \"bw encode failed: {}\"}}", String::from_utf8_lossy(&out.stderr)),
                    Err(e) => return format!("{{\"error\": \"bw encode failed: {e}\"}}"),
                }
            }
            Err(e) => return format!("{{\"error\": \"Failed to run bw encode: {e}\"}}"),
        };

        // Step 3: create item
        match self.run_bw_stdin(&["create", "item"], &encoded_output) {
            Ok(res) => {
                // Sync remote vault
                let _ = self.run_bw(&["sync"]);
                res
            }
            Err(e) => format!("{{\"error\": \"Failed to create item: {e}\"}}"),
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
