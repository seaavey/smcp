use super::*;

#[tool(tool_box)]
impl BitwardenService {
    #[tool(
        name = "bitwarden_set_password",
        description = "Set and save Bitwarden Master Password directly via MCP, testing unlock immediately"
    )]
    pub async fn set_password(&self, #[tool(aggr)] param: SetMasterPasswordParam) -> String {
        let pwd = param.master_password;
        if pwd.is_empty() {
            return serde_json::json!({
                "status": "error",
                "message": "Master password cannot be empty."
            })
            .to_string();
        }

        let password_for_unlock = pwd.clone();
        let output = match tokio::task::spawn_blocking(move || {
            BitwardenService::unlock_with_password(&password_for_unlock)
        })
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return serde_json::json!({
                    "status": "error",
                    "message": error
                })
                .to_string();
            }
            Err(error) => {
                return serde_json::json!({
                    "status": "error",
                    "message": format!("Bitwarden task failed: {error}")
                })
                .to_string();
            }
        };

        let new_session = String::from_utf8_lossy(&output).trim().to_string();
        if new_session.is_empty() {
            return serde_json::json!({
                "status": "error",
                "message": "Bitwarden unlock returned an empty session."
            })
            .to_string();
        }

        if let Ok(mut lock) = self.session.lock() {
            *lock = Some(new_session);
        }

        let creds_file = match BitwardenService::store_master_password(&pwd) {
            Ok(path) => path,
            Err(error) => {
                return serde_json::json!({
                    "status": "error",
                    "message": error
                })
                .to_string();
            }
        };

        serde_json::json!({
            "status": "success",
            "message": "Bitwarden master password verified, session unlocked, and saved!",
            "path": creds_file.display().to_string()
        })
        .to_string()
    }

    #[tool(
        name = "bitwarden_status",
        description = "Check Bitwarden status and sync status"
    )]
    pub async fn status(&self) -> String {
        match self.run_bw_async(vec!["status".into()]).await {
            Ok(res) => res,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(
        name = "bitwarden_sync",
        description = "Sync Bitwarden local cache with remote vault"
    )]
    pub async fn sync(&self) -> String {
        match self.run_bw_async(vec!["sync".into()]).await {
            Ok(_) => "{\"status\": \"synced\"}".into(),
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(
        name = "bitwarden_generate_password",
        description = "Generate a secure random password using Bitwarden CLI"
    )]
    pub async fn generate_password(&self, #[tool(aggr)] param: GeneratePasswordParam) -> String {
        let length_str = param.length.unwrap_or(24).to_string();
        let mut args = vec!["generate".to_string(), "--length".to_string(), length_str];

        let special = param.special.unwrap_or(true);
        let numbers = param.numbers.unwrap_or(true);
        let uppercase = param.uppercase.unwrap_or(true);
        let lowercase = param.lowercase.unwrap_or(true);

        if special {
            args.push("-s".to_string());
        }
        if uppercase {
            args.push("-u".to_string());
        }
        if lowercase {
            args.push("-l".to_string());
        }
        if numbers {
            args.push("-n".to_string());
        }

        let output =
            match tokio::task::spawn_blocking(move || Command::new("bw").args(args).output()).await
            {
                Ok(Ok(output)) if output.status.success() => {
                    String::from_utf8_lossy(&output.stdout).trim().to_string()
                }
                Ok(Ok(output)) => format!("Error: {}", String::from_utf8_lossy(&output.stderr)),
                Ok(Err(error)) => format!("Error running bw: {error}"),
                Err(error) => format!("Bitwarden task failed: {error}"),
            };

        serde_json::json!({
            "generated_password": output
        })
        .to_string()
    }

    #[tool(
        name = "bitwarden_create_login_item",
        description = "Create and save a new login item/credential into Bitwarden vault"
    )]
    pub async fn create_login_item(&self, #[tool(aggr)] param: CreateLoginItemParam) -> String {
        // Step 1: get template
        let template_raw = match self
            .run_bw_async(vec!["get".into(), "template".into(), "item".into()])
            .await
        {
            Ok(r) => r,
            Err(e) => return format!("{{\"error\": \"Failed to get item template: {e}\"}}"),
        };

        let login_template_raw = match self
            .run_bw_async(vec!["get".into(), "template".into(), "item.login".into()])
            .await
        {
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

        let payload_str = match serde_json::to_string(&item) {
            Ok(payload) => payload,
            Err(error) => {
                return serde_json::json!({
                    "error": format!("Failed to serialize login item: {error}")
                })
                .to_string()
            }
        };

        let encoded_output = match BitwardenService::encode_item_async(payload_str).await {
            Ok(output) => output,
            Err(error) => return serde_json::json!({"error": error}).to_string(),
        };

        // Step 3: create item
        match self
            .run_bw_stdin_async(vec!["create".into(), "item".into()], encoded_output)
            .await
        {
            Ok(res) => {
                // Sync remote vault
                let _ = self.run_bw_async(vec!["sync".into()]).await;
                res
            }
            Err(e) => format!("{{\"error\": \"Failed to create item: {e}\"}}"),
        }
    }

    #[tool(
        name = "bitwarden_list_items",
        description = "List vault items. Optionally filter with query keyword."
    )]
    pub async fn list_items(&self, #[tool(aggr)] param: SearchParam) -> String {
        let mut args = vec!["list", "items"];
        let q = param.query.unwrap_or_default();
        if !q.is_empty() {
            args.push("--search");
            args.push(&q);
        }

        match self
            .run_bw_async(args.into_iter().map(str::to_string).collect())
            .await
        {
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

    #[tool(
        name = "bitwarden_get_item",
        description = "Get details of a specific vault item by name or ID"
    )]
    pub async fn get_item(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self
            .run_bw_async(vec!["get".into(), "item".into(), param.id_or_name])
            .await
        {
            Ok(res) => res,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        }
    }

    #[tool(
        name = "bitwarden_get_password",
        description = "Get password directly for a specific vault item by name or ID"
    )]
    pub async fn get_password(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self
            .run_bw_async(vec!["get".into(), "password".into(), param.id_or_name])
            .await
        {
            Ok(res) => res.trim().to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        name = "bitwarden_get_totp",
        description = "Generate/get current TOTP 2FA code for a specific vault item"
    )]
    pub async fn get_totp(&self, #[tool(aggr)] param: GetItemParam) -> String {
        match self
            .run_bw_async(vec!["get".into(), "totp".into(), param.id_or_name])
            .await
        {
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
