use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub fn get_credentials_dir() -> PathBuf {
    let base = if let Ok(home) = env::var("HOME") {
        PathBuf::from(home)
    } else {
        PathBuf::from(".")
    };
    base.join(".config").join("credentials")
}

pub fn setup_gmail() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("     SMCP Setup: Google Workspace / Gmail");
    println!("==================================================");
    println!("Gmail authentication uses a Google App Password.");
    println!("Requirements:");
    println!(" 1. Enable 2-Step Verification on your Google Account:");
    println!("    https://myaccount.google.com/signinoptions/two-step-verification");
    println!(" 2. Create an App Password for 'Mail':");
    println!("    https://myaccount.google.com/apppasswords");
    println!("--------------------------------------------------");

    print!("Enter your Gmail address (e.g. user@gmail.com): ");
    io::stdout().flush()?;
    let mut email = String::new();
    io::stdin().read_line(&mut email)?;
    let email = email.trim().to_string();

    if email.is_empty() || !email.contains('@') {
        eprintln!("Error: Invalid email address.");
        return Ok(());
    }

    print!("Enter your 16-character App Password: ");
    io::stdout().flush()?;
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    let password = password.replace(' ', "").trim().to_string();

    if password.is_empty() {
        eprintln!("Error: App Password cannot be empty.");
        return Ok(());
    }

    println!("--------------------------------------------------");
    println!("Testing IMAP connection before saving...");

    let tls = native_tls::TlsConnector::builder().build()?;
    match imap::connect(("imap.gmail.com", 993), "imap.gmail.com", &tls) {
        Ok(client) => match client.login(&email, &password) {
            Ok(mut session) => {
                println!("✓ IMAP Login verified successfully!");
                let _ = session.logout();
            }
            Err((e, _)) => {
                eprintln!("✗ IMAP login failed: {e}");
                eprintln!("Please verify your 16-character App Password.");
                return Ok(());
            }
        },
        Err(e) => {
            eprintln!("✗ Failed to connect to imap.gmail.com: {e}");
            return Ok(());
        }
    }

    let creds_dir = get_credentials_dir();
    fs::create_dir_all(&creds_dir)?;

    let creds_file = creds_dir.join("workspace-google");
    let content = format!("GMAIL_EMAIL={email}\nGMAIL_APP_PASSWORD={password}\n");
    fs::write(&creds_file, content)?;

    println!("✓ Credentials saved to: {}", creds_file.display());
    println!("Setup completed successfully!");
    Ok(())
}

pub fn setup_bitwarden() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("     SMCP Setup: Bitwarden Vault");
    println!("==================================================");
    println!("Requirements:");
    println!(" 1. Ensure Bitwarden CLI (`bw`) is installed on your system.");
    println!(" 2. Run `bw login --apikey` if not logged in yet.");
    println!("--------------------------------------------------");

    print!("Enter Bitwarden Master Password: ");
    io::stdout().flush()?;
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    let password = password.trim().to_string();

    if password.is_empty() {
        eprintln!("Error: Master password cannot be empty.");
        return Ok(());
    }

    println!("--------------------------------------------------");
    println!("Testing Bitwarden CLI unlock before saving...");

    let output = std::process::Command::new("bw")
        .args(["unlock", &password, "--raw"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            println!("✓ Bitwarden vault unlocked successfully!");
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            eprintln!("✗ Bitwarden unlock failed: {err}");
            return Ok(());
        }
        Err(e) => {
            eprintln!("✗ Failed to run `bw`: {e}");
            eprintln!("Please ensure Bitwarden CLI is installed (`npm i -g @bitwarden/cli`).");
            return Ok(());
        }
    }

    let creds_dir = get_credentials_dir();
    fs::create_dir_all(&creds_dir)?;

    let creds_file = creds_dir.join("bitwarden_master_password");
    let content = format!("BITWARDEN_MASTER_PASSWORD={password}\n");
    fs::write(&creds_file, content)?;

    println!("✓ Credentials saved to: {}", creds_file.display());
    println!("Setup completed!");
    Ok(())
}
