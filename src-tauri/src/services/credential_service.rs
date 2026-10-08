use crate::credentials;

pub fn save_credential(name: &str, value: &str) -> Result<(), String> {
    if !credentials::ALL_CREDENTIALS.contains(&name) {
        return Err("Unsupported credential".to_string());
    }
    if value.trim().is_empty() {
        return Err("Credential value cannot be empty. Use delete_credential to remove a credential.".to_string());
    }
    credentials::save(name, value).map_err(|e| e.to_string())
}

pub fn delete_credential(name: &str) -> Result<(), String> {
    credentials::delete(name).map_err(|e| e.to_string())
}

pub fn credential_status(name: &str) -> Result<bool, String> {
    credentials::has(name).map_err(|e| e.to_string())
}

macro_rules! status_command {
    ($name:ident, $credential:expr) => {
        pub fn $name() -> Result<bool, String> {
            credential_status($credential)
        }
    };
}

status_command!(is_deepgram_configured, credentials::DEEPGRAM);
status_command!(is_gemini_configured, credentials::GEMINI);
status_command!(is_openai_configured, credentials::OPENAI);
status_command!(is_anthropic_configured, credentials::ANTHROPIC);
status_command!(is_deepseek_configured, credentials::DEEPSEEK);
status_command!(is_groq_configured, credentials::GROQ);
status_command!(is_openrouter_configured, credentials::OPENROUTER);
status_command!(is_nvidia_configured, credentials::NVIDIA);
status_command!(is_instagram_configured, credentials::INSTAGRAM);

pub fn is_youtube_configured() -> Result<bool, String> {
    [
        credentials::YOUTUBE_CLIENT_ID,
        credentials::YOUTUBE_CLIENT_SECRET,
        credentials::YOUTUBE_REFRESH_TOKEN,
    ]
    .iter()
    .try_fold(true, |configured, name| {
        credential_status(name).map(|present| configured && present)
    })
}

pub fn save_instagram_credentials(account_id: &str, access_token: &str) -> Result<(), String> {
    let acc_id = account_id.trim();
    let token = access_token.trim();

    if !acc_id.is_empty() {
        std::env::set_var("INSTAGRAM_ACCOUNT_ID", acc_id);
    }
    if !token.is_empty() {
        credentials::save(credentials::INSTAGRAM, token).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn save_youtube_credentials(client_id: &str, client_secret: &str, refresh_token: &str) -> Result<(), String> {
    let cid = client_id.trim();
    let sec = client_secret.trim();
    let tok = refresh_token.trim();

    if !cid.is_empty() {
        credentials::save(credentials::YOUTUBE_CLIENT_ID, cid).map_err(|e| e.to_string())?;
    }
    if !sec.is_empty() {
        credentials::save(credentials::YOUTUBE_CLIENT_SECRET, sec).map_err(|e| e.to_string())?;
    }
    if !tok.is_empty() {
        credentials::save(credentials::YOUTUBE_REFRESH_TOKEN, tok).map_err(|e| e.to_string())?;
    }

    Ok(())
}
