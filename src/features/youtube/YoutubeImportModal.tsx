import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Youtube,
  X,
  AlertTriangle,
  Loader2,
  ShieldAlert,
  Captions,
} from "lucide-react";
import { useAppError } from "../error/ErrorProvider";
import { AccessibleModal } from "../../components/AccessibleModal";

const CAPTION_STYLES = [
  { id: "hormozi-kinetic", label: "Hormozi Kinetic (Pro Karaoke ⚡)" },
  { id: "submagic-viral", label: "Submagic Viral (Auto-Emoji 💰)" },
  { id: "hormozi-punch", label: "Hormozi Punch (Auto-Emoji 🔥)" },
  { id: "neon-glow", label: "Neon Glow (Auto-Emoji 🚀)" },
  { id: "modern-box", label: "Modern Box (Clean Translucent)" },
  { id: "classic-outline", label: "Classic Outline (Bold Yellow Stroke)" },
  { id: "minimal-shadow", label: "Minimal Shadow (Pure White Elegant)" },
  { id: "vibrant-cyan", label: "Vibrant Cyan (Tech Gradient)" },
  { id: "vibrant-yellow-box", label: "Vibrant Yellow Box (High Contrast)" },
  { id: "vibrant-green", label: "Vibrant Green (Energy Neon)" },
  { id: "vibrant-red", label: "Vibrant Red (Dramatic Hook)" },
];

interface YoutubeImportModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (downloadedPath: string, captionStyle: string) => void;
  youtubeSaveDir: string;
  initialCaptionStyle?: string;
}

export function YoutubeImportModal({
  isOpen,
  onClose,
  onSuccess,
  youtubeSaveDir,
  initialCaptionStyle = "hormozi-kinetic",
}: YoutubeImportModalProps) {
  const [youtubeUrl, setYoutubeUrl] = useState("");
  const [captionStyle, setCaptionStyle] = useState(initialCaptionStyle);
  const [youtubeStatus, setYoutubeStatus] = useState<
    "idle" | "checking" | "warning" | "downloading"
  >("idle");
  const [youtubeWarningLicense, setYoutubeWarningLicense] = useState<
    string | null
  >(null);
  const [acknowledgedTos, setAcknowledgedTos] = useState(false);
  const { showError } = useAppError();

  if (!isOpen) return null;

  const handleCheckboxChange = (checked: boolean) => {
    setAcknowledgedTos(checked);
  };

  async function handleImport() {
    if (!youtubeUrl) return;
    setYoutubeStatus("checking");
    try {
      const result = await invoke<{ isSafe: boolean; license: string | null }>(
        "check_youtube_copyright",
        {
          url: youtubeUrl,
        }
      );

      if (!result.isSafe) {
        setYoutubeWarningLicense(result.license || "Standard YouTube License");
        setYoutubeStatus("warning");
        return;
      }
      await executeDownload();
    } catch (err) {
      showError("YouTube metadata check failed", { details: String(err) });
      setYoutubeStatus("idle");
    }
  }

  async function executeDownload() {
    setYoutubeStatus("downloading");
    try {
      const downloadedPath = await invoke<string>("download_youtube_video", {
        url: youtubeUrl,
        outputDir: youtubeSaveDir.trim() || null,
        userAcknowledged: acknowledgedTos,
      });
      const selectedStyle = captionStyle;
      setYoutubeUrl("");
      setAcknowledgedTos(false);
      setYoutubeStatus("idle");
      onClose();
      onSuccess(downloadedPath, selectedStyle);
    } catch (err) {
      showError("Failed to download YouTube video", { details: String(err) });
      setYoutubeStatus("idle");
    }
  }

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onClose}
      title="Import from YouTube"
      titleId="youtube-modal-title"
      dialogClassName="youtube-modal"
    >
      <div className="modal-header">
        <div className="modal-header-left">
          <div className="modal-icon-badge">
            <Youtube size={18} />
          </div>
          <div>
            <h3 id="youtube-modal-title">Import from YouTube</h3>
            <p>Download and convert a video directly into ClipOn</p>
          </div>
        </div>
        <button
          className="modal-close-btn"
          onClick={onClose}
          aria-label="Close dialog"
        >
          <X size={16} />
        </button>
      </div>

      <div className="youtube-modal-body">
        {/* Terms Compliance Banner */}
        <div className="yt-compliance-banner">
          <ShieldAlert
            size={16}
            color="var(--accent-cyan)"
            style={{ flexShrink: 0, marginTop: 2 }}
          />
          <div>
            <strong style={{ color: "var(--text)" }}>
              Notice & Terms of Service:
            </strong>
            <div style={{ marginTop: 2 }}>
              Ensure you have the right to download and use this content under
              YouTube’s Terms of Service and applicable copyright laws.
            </div>
          </div>
        </div>

        <input
          type="text"
          placeholder="https://www.youtube.com/watch?v=..."
          value={youtubeUrl}
          onChange={(e) => setYoutubeUrl(e.target.value)}
          disabled={youtubeStatus !== "idle" && youtubeStatus !== "warning"}
          className="youtube-url-input"
        />

        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <label
            style={{
              fontSize: 12,
              fontWeight: 600,
              color: "var(--text-muted)",
              display: "flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <Captions size={14} color="var(--accent-cyan)" />
            <span>Automated Subtitle Style</span>
          </label>
          <select
            className="youtube-url-input"
            value={captionStyle}
            onChange={(e) => setCaptionStyle(e.target.value)}
            disabled={youtubeStatus !== "idle" && youtubeStatus !== "warning"}
            style={{ cursor: "pointer", color: "var(--text)" }}
          >
            {CAPTION_STYLES.map((style) => (
              <option
                key={style.id}
                value={style.id}
                style={{ background: "var(--bg-card)", color: "var(--text)" }}
              >
                {style.label}
              </option>
            ))}
          </select>
        </div>

        {youtubeStatus === "warning" && (
          <div className="youtube-warning-box">
            <div className="warning-title">
              <AlertTriangle size={18} />
              <span>Copyright Advisory</span>
            </div>
            <p>
              This video is not explicitly marked with a Creative Commons
              license. Detected license:{" "}
              <strong>{youtubeWarningLicense}</strong>. Clipping and
              republishing copyrighted content may violate platform terms.
            </p>
          </div>
        )}

        <div
          style={{
            marginTop: 14,
            display: "flex",
            alignItems: "center",
            gap: 8,
          }}
        >
          <input
            type="checkbox"
            id="tos_ack"
            checked={acknowledgedTos}
            onChange={(e) => handleCheckboxChange(e.target.checked)}
            style={{ cursor: "pointer" }}
          />
          <label
            htmlFor="tos_ack"
            style={{
              fontSize: 12,
              color: "#cbd5e1",
              cursor: "pointer",
              userSelect: "none",
            }}
          >
            I confirm I have permission or legal right to use this content
          </label>
        </div>
      </div>

      <div className="modal-footer">
        <button
          className="studio-btn secondary"
          onClick={() => {
            onClose();
            setYoutubeUrl("");
            setAcknowledgedTos(false);
            setYoutubeStatus("idle");
          }}
          disabled={
            youtubeStatus === "checking" || youtubeStatus === "downloading"
          }
        >
          Cancel
        </button>
        {youtubeStatus === "warning" ? (
          <button
            className="studio-btn danger"
            onClick={executeDownload}
            disabled={!acknowledgedTos}
          >
            Proceed Anyway
          </button>
        ) : (
          <button
            className="studio-btn primary"
            onClick={handleImport}
            disabled={
              !youtubeUrl || !acknowledgedTos || youtubeStatus !== "idle"
            }
          >
            {youtubeStatus === "checking" ? (
              <>
                <Loader2 className="spin" size={14} /> Checking...
              </>
            ) : youtubeStatus === "downloading" ? (
              <>
                <Loader2 className="spin" size={14} /> Downloading...
              </>
            ) : (
              "Download & Import"
            )}
          </button>
        )}
      </div>
    </AccessibleModal>
  );
}
