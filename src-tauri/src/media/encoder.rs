use serde::{Deserialize, Serialize};
use std::process::Command;

use super::ffmpeg::resolve_binary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCapabilities {
    pub videotoolbox: bool,
    pub h264: bool,
    pub hevc: bool,
}

static CACHED_CAPS: std::sync::OnceLock<HardwareCapabilities> = std::sync::OnceLock::new();

pub fn detect_hardware_capabilities() -> HardwareCapabilities {
    CACHED_CAPS
        .get_or_init(|| {
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
        })
        .clone()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hardware_capabilities_cached() {
        let caps1 = detect_hardware_capabilities();
        let caps2 = detect_hardware_capabilities();
        assert_eq!(caps1.videotoolbox, caps2.videotoolbox);
        assert_eq!(caps1.h264, caps2.h264);
        assert_eq!(caps1.hevc, caps2.hevc);
    }
}
