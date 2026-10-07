use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

const SERVICE_NAME: &str = "com.clipon.app";

pub const DEEPGRAM: &str = "deepgram";
pub const GEMINI: &str = "gemini";
pub const OPENAI: &str = "openai";
pub const ANTHROPIC: &str = "anthropic";
pub const DEEPSEEK: &str = "deepseek";
pub const GROQ: &str = "groq";
pub const OPENROUTER: &str = "openrouter";
pub const INSTAGRAM: &str = "instagram";
pub const NVIDIA: &str = "nvidia";
pub const NVIDIA_FUNCTION_ID: &str = "nvidia_function_id";
pub const YOUTUBE_CLIENT_ID: &str = "youtube_client_id";
pub const YOUTUBE_CLIENT_SECRET: &str = "youtube_client_secret";
pub const YOUTUBE_REFRESH_TOKEN: &str = "youtube_refresh_token";

/// Maps canonical credential identifier to its standard environment variable name.
pub fn env_var_for_credential(name: &str) -> Option<&'static str> {
    match name {
        DEEPGRAM => Some("DEEPGRAM_API_KEY"),
        GEMINI => Some("GEMINI_API_KEY"),
        OPENAI => Some("OPENAI_API_KEY"),
        ANTHROPIC => Some("ANTHROPIC_API_KEY"),
        DEEPSEEK => Some("DEEPSEEK_API_KEY"),
        GROQ => Some("GROQ_API_KEY"),
        OPENROUTER => Some("OPENROUTER_API_KEY"),
        INSTAGRAM => Some("INSTAGRAM_ACCESS_TOKEN"),
        NVIDIA => Some("NVIDIA_API_KEY"),
        NVIDIA_FUNCTION_ID => Some("NVIDIA_ASD_FUNCTION_ID"),
        YOUTUBE_CLIENT_ID => Some("YOUTUBE_CLIENT_ID"),
        YOUTUBE_CLIENT_SECRET => Some("YOUTUBE_CLIENT_SECRET"),
        YOUTUBE_REFRESH_TOKEN => Some("YOUTUBE_REFRESH_TOKEN"),
        _ => None,
    }
}

/// Resolves the secure file keystore path inside the app's persistent data directory.
fn credentials_file_path() -> Option<PathBuf> {
    if let Some(data_dir) = dirs::data_dir() {
        let p = data_dir.join("com.clipon.desktop").join("credentials.json");
        return Some(p);
    }
    dirs::home_dir().map(|h| h.join(".clipon").join("credentials.json"))
}

/// Reads all saved credentials from the local restricted file keystore.
fn read_file_keystore() -> HashMap<String, String> {
    let Some(path) = credentials_file_path() else {
        return HashMap::new();
    };
    if !path.exists() {
        return HashMap::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

/// Writes all credentials to the local restricted file keystore (chmod 0600 on Unix).
fn write_file_keystore(store: &HashMap<String, String>) -> Result<()> {
    let Some(path) = credentials_file_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(store)?;
    std::fs::write(&path, json)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Updates or appends a KEY=VALUE entry in an existing .env file.
fn update_env_file_key(path: &std::path::Path, key: &str, value: Option<&str>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(path)?;
    let mut found = false;
    let mut lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{key}=")) || trimmed == key {
            found = true;
            if let Some(v) = value {
                lines.push(format!("{key}={v}"));
            }
        } else {
            lines.push(line.to_string());
        }
    }

    if !found {
        if let Some(v) = value {
            lines.push(format!("{key}={v}"));
        }
    }

    let mut new_content = lines.join("\n");
    if !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    std::fs::write(path, new_content)?;
    Ok(())
}

/// Propagates saved credential to active .env files across data_dir and dev roots.
fn sync_to_env_files(env_var: &str, value: Option<&str>) {
    let candidate_paths = [
        dirs::data_dir().map(|d| d.join("com.clipon.desktop").join(".env")),
        Some(PathBuf::from(".env")),
        Some(PathBuf::from("../.env")),
    ];

    for path_opt in candidate_paths.into_iter().flatten() {
        if path_opt.exists() {
            let _ = update_env_file_key(&path_opt, env_var, value);
        }
    }
}

/// Resolves a credential with zero system-password prompts:
/// 1. Process environment (loaded from .env)
/// 2. Local restricted file keystore (credentials.json)
/// 3. OS Keychain (gracefully handled without bubbling errors or prompting)
pub fn get(name: &str) -> Result<Option<String>> {
    // 1. Process environment (loaded from .env or previous save)
    if let Some(env_var) = env_var_for_credential(name) {
        if let Ok(val) = std::env::var(env_var) {
            let clean = val.trim().to_string();
            if !clean.is_empty() {
                return Ok(Some(clean));
            }
        }
    }

    // 2. Local secure file keystore
    let file_store = read_file_keystore();
    if let Some(val) = file_store.get(name) {
        let clean = val.trim().to_string();
        if !clean.is_empty() {
            if let Some(env_var) = env_var_for_credential(name) {
                std::env::set_var(env_var, &clean);
            }
            return Ok(Some(clean));
        }
    }

    // 3. Fallback to OS Keychain only if not found in env or file store
    // Any error (such as user canceling a prompt, ACL rejection, or locked keychain)
    // is safely treated as None so it NEVER crashes the app or resets the settings UI.
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, name) {
            if let Ok(value) = entry.get_password() {
                let clean = value.trim().to_string();
                if !clean.is_empty() {
                    return Ok(Some(clean));
                }
            }
        }
    }

    Ok(None)
}

/// Securely persists a credential into:
/// 1. Current process environment
/// 2. Local file keystore (chmod 0600)
/// 3. .env files
/// 4. OS Keychain (graceful/silent fallback)
pub fn save(name: &str, value: &str) -> Result<()> {
    let clean = value.trim();
    if clean.is_empty() {
        return Ok(());
    }

    // 1. Set environment variable
    if let Some(env_var) = env_var_for_credential(name) {
        std::env::set_var(env_var, clean);
        sync_to_env_files(env_var, Some(clean));
    }

    // 2. Persist in file keystore
    let mut store = read_file_keystore();
    store.insert(name.to_string(), clean.to_string());
    let _ = write_file_keystore(&store);

    // 3. Silently attempt OS Keychain (ignore errors)
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, name) {
            let _ = entry.set_password(clean);
        }
    }

    Ok(())
}

/// Deletes a credential from all storage tiers.
pub fn delete(name: &str) -> Result<()> {
    if let Some(env_var) = env_var_for_credential(name) {
        std::env::remove_var(env_var);
        sync_to_env_files(env_var, None);
    }

    let mut store = read_file_keystore();
    if store.remove(name).is_some() {
        let _ = write_file_keystore(&store);
    }

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, name) {
            let _ = entry.delete_credential();
        }
    }

    Ok(())
}

/// Returns whether a non-empty credential exists for the given name.
pub fn has(name: &str) -> Result<bool> {
    Ok(get(name)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nvidia_credential_interface() {
        let e = keyring::Entry::new(SERVICE_NAME, NVIDIA);
        assert!(e.is_ok(), "Keyring entry should construct cleanly: {:?}", e.err());
    }

    #[test]
    fn test_env_var_mapping() {
        assert_eq!(env_var_for_credential(DEEPGRAM), Some("DEEPGRAM_API_KEY"));
        assert_eq!(env_var_for_credential(GEMINI), Some("GEMINI_API_KEY"));
        assert_eq!(env_var_for_credential(NVIDIA), Some("NVIDIA_API_KEY"));
        assert_eq!(env_var_for_credential(NVIDIA_FUNCTION_ID), Some("NVIDIA_ASD_FUNCTION_ID"));
        assert_eq!(env_var_for_credential(YOUTUBE_CLIENT_ID), Some("YOUTUBE_CLIENT_ID"));
        assert_eq!(env_var_for_credential(YOUTUBE_CLIENT_SECRET), Some("YOUTUBE_CLIENT_SECRET"));
        assert_eq!(env_var_for_credential(YOUTUBE_REFRESH_TOKEN), Some("YOUTUBE_REFRESH_TOKEN"));
    }
}
