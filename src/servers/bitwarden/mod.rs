use rmcp::{
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct BitwardenService {
    session: Arc<Mutex<Option<String>>>,
}

mod models;
mod tools;

pub use models::*;

impl BitwardenService {
    fn credential_path() -> Option<PathBuf> {
        env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".config/credentials/bitwarden_master_password"))
    }

    fn write_private_file(path: &PathBuf, content: &str) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Credential path has no parent directory".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create credentials directory: {e}"))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| format!("Failed to open credential file: {e}"))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to secure credential file: {e}"))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write credential file: {e}"))
    }

    fn resolve_master_password() -> Result<String, String> {
        if let Ok(val) = env::var("BW_PASSWORD") {
            if !val.is_empty() {
                return Ok(val);
            }
        }
        if let Ok(val) = env::var("BITWARDEN_MASTER_PASSWORD") {
            if !val.is_empty() {
                return Ok(val);
            }
        }

        if let Some(path) = Self::credential_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(val) = trimmed.strip_prefix("BITWARDEN_MASTER_PASSWORD=") {
                            if !val.is_empty() {
                                return Ok(val.to_string());
                            }
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
        if let Some(session) = self.session.lock().map_err(|e| e.to_string())?.clone() {
            return Ok(session);
        }

        if let Ok(session) = env::var("BW_SESSION") {
            if !session.trim().is_empty() {
                let session = session.trim().to_string();
                *self.session.lock().map_err(|e| e.to_string())? = Some(session.clone());
                return Ok(session);
            }
        }

        let password = Self::resolve_master_password()?;
        let output = Self::unlock_with_password(&password)?;
        let session = String::from_utf8_lossy(&output).trim().to_string();
        if session.is_empty() {
            return Err("bw unlock returned an empty session".into());
        }
        *self.session.lock().map_err(|e| e.to_string())? = Some(session.clone());
        Ok(session)
    }

    fn unlock_with_password(password: &str) -> Result<Vec<u8>, String> {
        let child = Command::new("bw")
            .args(["unlock", "--passwordenv", "BW_PASSWORD", "--raw"])
            .env("BW_PASSWORD", password)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to execute bw unlock: {e}"))?;
        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait for bw unlock: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "bw unlock failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(output.stdout)
    }

    fn store_master_password(password: &str) -> Result<PathBuf, String> {
        let path = Self::credential_path().ok_or_else(|| "HOME is not configured".to_string())?;
        Self::write_private_file(&path, &format!("BITWARDEN_MASTER_PASSWORD={password}\n"))?;
        Ok(path)
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

    async fn run_bw_async(&self, args: Vec<String>) -> Result<String, String> {
        let service = self.clone();
        tokio::task::spawn_blocking(move || {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            service.run_bw(&args)
        })
        .await
        .map_err(|error| format!("Bitwarden task failed: {error}"))?
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
            stdin
                .write_all(input.as_bytes())
                .map_err(|e| format!("Write stdin failed: {e}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Wait bw failed: {e}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("bw error: {err}"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn run_bw_stdin_async(&self, args: Vec<String>, input: String) -> Result<String, String> {
        let service = self.clone();
        tokio::task::spawn_blocking(move || {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            service.run_bw_stdin(&args, &input)
        })
        .await
        .map_err(|error| format!("Bitwarden task failed: {error}"))?
    }

    async fn encode_item_async(payload: String) -> Result<String, String> {
        tokio::task::spawn_blocking(move || {
            let mut child = Command::new("bw")
                .arg("encode")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|error| format!("Failed to run bw encode: {error}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(payload.as_bytes())
                    .map_err(|error| format!("Failed to write bw encode input: {error}"))?;
            }
            let output = child
                .wait_with_output()
                .map_err(|error| format!("Failed to wait for bw encode: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "bw encode failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .await
        .map_err(|error| format!("Bitwarden task failed: {error}"))?
    }
}
