use anyhow::{Context, Result};

const SERVICE_NAME: &str = "com.clipon.app";

pub const DEEPGRAM: &str = "deepgram";
pub const GEMINI: &str = "gemini";
pub const OPENAI: &str = "openai";
pub const ANTHROPIC: &str = "anthropic";
pub const DEEPSEEK: &str = "deepseek";
pub const GROQ: &str = "groq";
pub const OPENROUTER: &str = "openrouter";

pub fn get(name: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE_NAME, name)
        .with_context(|| format!("creating secure credential entry for {name}"))?;

    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading secure credential {name}")),
    }
}

pub fn save(name: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, name)
        .with_context(|| format!("creating secure credential entry for {name}"))?;
    entry
        .set_password(value.trim())
        .with_context(|| format!("saving secure credential {name}"))
}

pub fn delete(name: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, name)
        .with_context(|| format!("creating secure credential entry for {name}"))?;

    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error).with_context(|| format!("deleting secure credential {name}")),
    }
}

pub fn has(name: &str) -> Result<bool> {
    Ok(get(name)?.is_some())
}
