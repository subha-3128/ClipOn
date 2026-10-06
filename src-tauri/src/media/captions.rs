use std::path::Path;

pub fn escape_ffmpeg_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "'\\''")
}

pub fn build_ass_filter(ass_path: &Path) -> Option<String> {
    if ass_path.exists() {
        let escaped = escape_ffmpeg_path(ass_path);
        Some(format!("ass='{}'", escaped))
    } else {
        None
    }
}
