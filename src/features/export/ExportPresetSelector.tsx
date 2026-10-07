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
    <div className="export-preset-selector">
      <label className="export-preset-label">
        Export Preset (1080×1920 Vertical)
      </label>
      <div className="export-preset-grid">
        {PRESETS.map((p) => {
          const isSelected = selectedPreset === p.platform;
          return (
            <button
              key={p.platform}
              type="button"
              onClick={() => onSelectPreset(p.platform)}
              className={`export-preset-btn ${isSelected ? "selected" : ""}`}
            >
              <div className="export-preset-btn-header">
                {p.platform === "instagram_reels" ? (
                  <Instagram
                    size={13}
                    color={isSelected ? "var(--accent-cyan)" : "var(--muted)"}
                  />
                ) : p.platform === "youtube_shorts" ? (
                  <Youtube
                    size={13}
                    color={isSelected ? "var(--accent-cyan)" : "var(--muted)"}
                  />
                ) : (
                  <Video
                    size={13}
                    color={isSelected ? "var(--accent-cyan)" : "var(--muted)"}
                  />
                )}
                <span className="export-preset-btn-name">{p.label}</span>
              </div>
              <span className="export-preset-btn-meta">
                {p.fps}fps • {p.bitrateKbps / 1000}M
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
