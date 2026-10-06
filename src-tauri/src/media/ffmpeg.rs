use std::{path::Path, process::Command};

pub fn resolve_binary(name: &str) -> String {
    for prefix in &[
        "/opt/homebrew/bin",
        "/opt/homebrew/opt/ffmpeg-full/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
    ] {
        let p = Path::new(prefix).join(name);
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    name.to_string()
}

pub fn command_exists(name: &str) -> bool {
    let bin = resolve_binary(name);
    Command::new(bin).arg("-version").output().is_ok()
}

pub fn check_binary_available(name: &str) -> anyhow::Result<String> {
    if !command_exists(name) {
        let hint = match name {
            "ffmpeg" | "ffprobe" => "Install via Homebrew: 'brew install ffmpeg'",
            "yt-dlp" => "Install via Homebrew: 'brew install yt-dlp'",
            _ => "Please ensure it is installed and accessible on PATH.",
        };
        return Err(anyhow::anyhow!("Required media tool '{}' is missing. {}", name, hint));
    }
    Ok(resolve_binary(name))
}

#[allow(dead_code)]
pub fn create_ffmpeg_command() -> Command {
    let mut cmd = Command::new(resolve_binary("ffmpeg"));
    cmd.arg("-nostdin");
    cmd
}
