use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExportPlatform {
    InstagramReels,
    YouTubeShorts,
    TikTok,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPreset {
    pub platform: ExportPlatform,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub video_bitrate_kbps: u32,
    pub audio_bitrate_kbps: u32,
}

impl Default for OutputPreset {
    fn default() -> Self {
        Self::instagram_reels()
    }
}

#[allow(dead_code)]
impl OutputPreset {
    pub fn instagram_reels() -> Self {
        Self {
            platform: ExportPlatform::InstagramReels,
            width: 1080,
            height: 1920,
            fps: 30,
            video_bitrate_kbps: 6000,
            audio_bitrate_kbps: 192,
        }
    }

    pub fn youtube_shorts() -> Self {
        Self {
            platform: ExportPlatform::YouTubeShorts,
            width: 1080,
            height: 1920,
            fps: 60,
            video_bitrate_kbps: 8000,
            audio_bitrate_kbps: 256,
        }
    }

    pub fn tiktok() -> Self {
        Self {
            platform: ExportPlatform::TikTok,
            width: 1080,
            height: 1920,
            fps: 30,
            video_bitrate_kbps: 6000,
            audio_bitrate_kbps: 192,
        }
    }

    pub fn custom(
        width: u32,
        height: u32,
        fps: u32,
        video_bitrate_kbps: u32,
        audio_bitrate_kbps: u32,
    ) -> Self {
        Self {
            platform: ExportPlatform::Custom,
            width,
            height,
            fps,
            video_bitrate_kbps,
            audio_bitrate_kbps,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "youtube" | "shorts" | "youtube_shorts" => Self::youtube_shorts(),
            "tiktok" => Self::tiktok(),
            _ => Self::instagram_reels(),
        }
    }
}
