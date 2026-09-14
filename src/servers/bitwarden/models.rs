use schemars::JsonSchema;
use serde::Deserialize;

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
