use crate::credentials;

pub fn save_credential(name: &str, value: &str) -> Result<(), String> {
    let allowed = [
        credentials::DEEPGRAM,
        credentials::GEMINI,
        credentials::OPENAI,
        credentials::ANTHROPIC,
        credentials::DEEPSEEK,
        credentials::GROQ,
        credentials::OPENROUTER,
        credentials::INSTAGRAM,
        credentials::NVIDIA,
        credentials::NVIDIA_FUNCTION_ID,
        credentials::YOUTUBE_CLIENT_ID,
        credentials::YOUTUBE_CLIENT_SECRET,
        credentials::YOUTUBE_REFRESH_TOKEN,
    ];
    if !allowed.contains(&name) {
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
