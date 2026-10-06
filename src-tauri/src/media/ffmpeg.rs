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

#[allow(dead_code)]
pub fn create_ffmpeg_command() -> Command {
    let mut cmd = Command::new(resolve_binary("ffmpeg"));
    cmd.arg("-nostdin");
    cmd
}
