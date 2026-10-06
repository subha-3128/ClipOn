use std::path::Path;

pub fn escape_ffmpeg_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "'\\''")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

pub fn build_ass_filter(ass_path: &Path) -> Option<String> {
    if ass_path.exists() {
        let escaped = escape_ffmpeg_path(ass_path);
        Some(format!("ass='{}'", escaped))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_escape_ffmpeg_path_simple() {
        let p = PathBuf::from("/tmp/video.ass");
        assert_eq!(escape_ffmpeg_path(&p), "/tmp/video.ass");
    }

    #[test]
    fn test_escape_ffmpeg_path_windows_and_colons() {
        let p = PathBuf::from("C:\\Users\\Bob:Project\\captions.ass");
        let escaped = escape_ffmpeg_path(&p);
        assert!(escaped.contains("C\\:"));
        assert!(escaped.contains("Bob\\:Project"));
    }

    #[test]
    fn test_escape_ffmpeg_path_spaces_and_quotes() {
        let p = PathBuf::from("/Users/John's Video/test captions.ass");
        let escaped = escape_ffmpeg_path(&p);
        assert_eq!(escaped, "/Users/John'\\''s Video/test captions.ass");
    }

    #[test]
    fn test_escape_ffmpeg_path_brackets() {
        let p = PathBuf::from("/media/[2024] Episode 01 [1080p].ass");
        let escaped = escape_ffmpeg_path(&p);
        assert_eq!(escaped, "/media/\\[2024\\] Episode 01 \\[1080p\\].ass");
    }

    #[test]
    fn test_escape_ffmpeg_path_unicode() {
        let p = PathBuf::from("/videos/日本語_podcast/字幕.ass");
        let escaped = escape_ffmpeg_path(&p);
        assert_eq!(escaped, "/videos/日本語_podcast/字幕.ass");
    }
}
