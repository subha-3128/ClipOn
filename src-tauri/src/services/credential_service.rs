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
