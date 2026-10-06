import { Instagram, Youtube, Video } from "lucide-react";
import type { ExportPresetConfig, ExportPresetPlatform } from "../../types";

const PRESETS: ExportPresetConfig[] = [
  {
    platform: "instagram_reels",
    label: "Instagram Reels",
    width: 1080,
    height: 1920,
    aspectRatio: "9:16",
    fps: 30,
    bitrateKbps: 6000,
    iconName: "instagram",
  },
  {
    platform: "youtube_shorts",
    label: "YouTube Shorts",
    width: 1080,
    height: 1920,
    aspectRatio: "9:16",
    fps: 60,
    bitrateKbps: 8000,
    iconName: "youtube",
  },
  {
    platform: "tiktok",
    label: "TikTok",
    width: 1080,
    height: 1920,
    aspectRatio: "9:16",
    fps: 30,
    bitrateKbps: 6000,
    iconName: "tiktok",
  },
];

type ExportPresetSelectorProps = {
  selectedPreset: ExportPresetPlatform;
  onSelectPreset: (preset: ExportPresetPlatform) => void;
};

export function ExportPresetSelector({
  selectedPreset,
  onSelectPreset,
}: ExportPresetSelectorProps) {
  return (
    <div className="export-preset-selector" style={{ margin: "10px 0" }}>
      <label
        style={{
          fontSize: "11px",
          fontWeight: "600",
          color: "#94a3b8",
          textTransform: "uppercase",
          display: "block",
          marginBottom: "6px",
        }}
      >
        Export Preset (1080×1920 Vertical)
      </label>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: "8px",
        }}
      >
        {PRESETS.map((p) => {
          const isSelected = selectedPreset === p.platform;
          return (
            <button
              key={p.platform}
              type="button"
              onClick={() => onSelectPreset(p.platform)}
              style={{
                background: isSelected
                  ? "rgba(56, 189, 248, 0.15)"
                  : "rgba(30, 41, 59, 0.5)",
                border: isSelected
                  ? "1px solid #38bdf8"
                  : "1px solid rgba(255, 255, 255, 0.08)",
                borderRadius: "8px",
                padding: "8px 10px",
                textAlign: "left",
                cursor: "pointer",
                display: "flex",
                flexDirection: "column",
                gap: "3px",
              }}
            >
              <div
                style={{ display: "flex", alignItems: "center", gap: "6px" }}
              >
                {p.platform === "instagram_reels" ? (
                  <Instagram
                    size={13}
                    color={isSelected ? "#38bdf8" : "#94a3b8"}
                  />
                ) : p.platform === "youtube_shorts" ? (
                  <Youtube
                    size={13}
                    color={isSelected ? "#38bdf8" : "#94a3b8"}
                  />
                ) : (
                  <Video size={13} color={isSelected ? "#38bdf8" : "#94a3b8"} />
                )}
                <span
                  style={{
                    fontSize: "11px",
                    fontWeight: "600",
                    color: isSelected ? "#f8fafc" : "#cbd5e1",
                  }}
                >
                  {p.label}
                </span>
              </div>
              <span style={{ fontSize: "10px", color: "#64748b" }}>
                {p.fps}fps • {p.bitrateKbps / 1000}M
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
