import { Zap } from "lucide-react";
import { EnvironmentStatus } from "../../types";

interface StatusBarProps {
  environment: EnvironmentStatus | null;
  canUseCloudKey: boolean;
}

export function StatusBar({ environment, canUseCloudKey }: StatusBarProps) {
  return (
    <footer className="status-bar">
      <div className="status-bar-left">
        <span className="system-dot" />
        <span className="system-text">System Ready</span>
        {environment?.hasHardwareAccel && (
          <span
            className="accel-pill"
            title="Apple Silicon VideoToolbox Hardware Acceleration"
          >
            <Zap size={11} /> VideoToolbox GPU Active
          </span>
        )}
      </div>

      <div className="status-bar-right">
        <span
          className={`status-tag ${environment?.hasFfmpeg ? "active" : ""}`}
        >
          ffmpeg
        </span>
        <span
          className={`status-tag ${environment?.hasFfprobe ? "active" : ""}`}
        >
          ffprobe
        </span>
        <span className={`status-tag ${environment?.hasYtdlp ? "active" : ""}`}>
          yt-dlp
        </span>
        <span
          className={`status-tag ${environment?.hasLocalWhisperModel ? "active" : ""}`}
        >
          Whisper
        </span>
        <span
          className={`status-tag ${environment?.hasOllama ? "active" : ""}`}
        >
          Ollama
        </span>
        <span className={`status-tag ${canUseCloudKey ? "active" : ""}`}>
          Deepgram
        </span>
      </div>
    </footer>
  );
}
