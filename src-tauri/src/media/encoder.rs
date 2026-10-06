use serde::{Deserialize, Serialize};
use std::process::Command;

use super::ffmpeg::resolve_binary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCapabilities {
    pub videotoolbox: bool,
    pub h264: bool,
    pub hevc: bool,
}

pub fn detect_hardware_capabilities() -> HardwareCapabilities {
    #[cfg(target_os = "macos")]
    {
        let bin = resolve_binary("ffmpeg");
        if let Ok(output) = Command::new(bin).args(["-encoders"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let has_h264_vt = stdout.contains("h264_videotoolbox");
            let has_hevc_vt = stdout.contains("hevc_videotoolbox");
            return HardwareCapabilities {
                videotoolbox: has_h264_vt || has_hevc_vt,
                h264: has_h264_vt,
                hevc: has_hevc_vt,
            };
        }
    }

    HardwareCapabilities {
        videotoolbox: false,
        h264: false,
        hevc: false,
    }
}

pub fn supports_videotoolbox() -> bool {
    detect_hardware_capabilities().h264
}

pub fn apply_video_encoder_args(cmd: &mut Command, use_videotoolbox: bool, bitrate_k: u32) {
    if use_videotoolbox {
        cmd.args([
            "-c:v",
            "h264_videotoolbox",
            "-b:v",
            &format!("{bitrate_k}k"),
            "-pix_fmt",
            "yuv420p",
        ]);
    } else {
        cmd.args([
            "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p",
        ]);
    }
}
