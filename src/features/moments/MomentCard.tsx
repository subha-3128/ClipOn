import {
  CheckSquare,
  Square,
  Instagram,
  Youtube,
  Loader2,
  AlertTriangle,
  Sparkles,
  Scissors,
  FolderOpen,
} from "lucide-react";
import { Candidate, Clip, InstagramPost, YouTubePost } from "../../types";

interface MomentCardProps {
  candidate: Candidate;
  clip?: Clip;
  igPost?: InstagramPost;
  ytPost?: YouTubePost;
  isCuttingThis: boolean;
  isPublishingThis: boolean;
  isPublishingToYoutube?: boolean;
  formatTime: (sec: number) => string;
  onToggleSelect: (id: string) => void;
  onOpenSocialKit: (candidate: Candidate) => void;
  onCutCandidate: (candidateId: string) => void;
  onOpenFolder: (path: string) => void;
  onPublishToInstagram: (candidateId: string) => void;
  onPublishToYouTube: (candidateId: string) => void;
  hasFfmpeg: boolean;
  isBusy: boolean;
}

export function MomentCard({
  candidate,
  clip,
  igPost,
  ytPost,
  isCuttingThis,
  isPublishingThis,
  isPublishingToYoutube = false,
  formatTime,
  onToggleSelect,
  onOpenSocialKit,
  onCutCandidate,
  onOpenFolder,
  onPublishToInstagram,
  onPublishToYouTube,
  hasFfmpeg,
  isBusy,
}: MomentCardProps) {
  const isCut = clip?.status === "done" && Boolean(clip.outputPath);

  return (
    <article
      className={`moment-candidate-card ${candidate.selected ? "selected" : ""}`}
    >
      <div className="moment-card-header">
        <div className="moment-card-header-left">
          <button
            className="checkbox-toggle-btn"
            onClick={() => onToggleSelect(candidate.id)}
            title="Toggle clip selection"
          >
            {candidate.selected ? (
              <CheckSquare size={17} className="checked" />
            ) : (
              <Square size={17} />
            )}
          </button>
          <span className="moment-rank-badge">#{candidate.rank}</span>
          <span className="moment-score-badge">
            {Math.round(
              candidate.score > 1 ? candidate.score : candidate.score * 100
            )}
            % Viral Score
          </span>
          {candidate.rationale.includes("High Audio Energy") && (
            <span
              className="moment-audio-energy-badge"
              title="High-Energy Audio Hook & Vocal Surge"
            >
              ⚡ High Audio Energy
            </span>
          )}
          {candidate.rationale.includes("Active Speaker: NVIDIA ASD") && (
            <span
              className="moment-asd-badge nvidia"
              title="Active Speaker Provider: NVIDIA ASD NIM (gRPC / NVCF neural inference)"
            >
              ⚡ NVIDIA ASD
            </span>
          )}
          {candidate.rationale.includes("Active Speaker: Local Fallback") && (
            <span
              className="moment-asd-badge local-fallback"
              title="Active Speaker Provider: Local Fallback (Apple Vision + Diarization Fusion)"
            >
              ⚠️ Local Fallback
            </span>
          )}
          <span className="moment-duration-badge">
            {formatTime(candidate.startSec)} - {formatTime(candidate.endSec)} (
            {Math.round(candidate.endSec - candidate.startSec)}s)
          </span>
        </div>

        <div className="moment-card-header-right">
          {(() => {
            if (igPost?.status === "published") {
              return (
                <a
                  href={igPost.postUrl || "#"}
                  target="_blank"
                  rel="noreferrer"
                  className="instagram-status-pill published"
                  onClick={(e) => {
                    if (igPost.postUrl) {
                      e.preventDefault();
                      onOpenFolder(igPost.postUrl);
                    }
                  }}
                  title="View live Instagram Reel"
                >
                  <Instagram size={11} />
                  <span>Reel Published ↗</span>
                </a>
              );
            }
            if (igPost?.status === "publishing" || isPublishingThis) {
              return (
                <span className="instagram-status-pill publishing">
                  <Loader2 className="spin" size={11} />
                  <span>Posting to IG...</span>
                </span>
              );
            }
            if (igPost?.status === "failed") {
              return (
                <span
                  className="instagram-status-pill failed"
                  title={igPost.errorMessage || "Failed"}
                >
                  <AlertTriangle size={11} />
                  <span>IG Failed</span>
                </span>
              );
            }
            return null;
          })()}
          {(() => {
            if (ytPost?.status === "published") {
              return (
                <a
                  href={ytPost.videoUrl || "#"}
                  target="_blank"
                  rel="noreferrer"
                  className="youtube-status-pill published"
                  onClick={(e) => {
                    if (ytPost.videoUrl) {
                      e.preventDefault();
                      onOpenFolder(ytPost.videoUrl);
                    }
                  }}
                  title="View live YouTube Short"
                >
                  <Youtube size={11} />
                  <span>Shorts Live ↗</span>
                </a>
              );
            }
            if (ytPost?.status === "publishing" || isPublishingToYoutube) {
              return (
                <span className="youtube-status-pill publishing">
                  <Loader2 className="spin" size={11} />
                  <span>Uploading to Shorts...</span>
                </span>
              );
            }
            if (ytPost?.status === "failed") {
              return (
                <span
                  className="youtube-status-pill failed"
                  title={ytPost.errorMessage || "Failed"}
                >
                  <AlertTriangle size={11} />
                  <span>Shorts Failed</span>
                </span>
              );
            }
            return null;
          })()}
          <span
            className={`moment-render-status ${
              isCut ? "ready" : clip?.status === "error" ? "error" : "pending"
            }`}
          >
            {isCuttingThis
              ? "Rendering..."
              : isCut
                ? "Ready"
                : clip?.status === "error"
                  ? "Failed"
                  : "Pending"}
          </span>
        </div>
      </div>

      <div className="moment-card-body">
        <h4 className="moment-hook">{candidate.hook}</h4>
        <p className="moment-rationale">{candidate.rationale}</p>

        {candidate.qualityScore && (
          <div
            className="moment-quality-breakdown"
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: "6px",
              marginTop: "6px",
              marginBottom: "8px",
            }}
          >
            {candidate.qualityScore.hook !== null && (
              <span
                className="quality-metric-tag"
                title="First 2s scroll-stopping hook strength"
                style={{
                  fontSize: "11px",
                  padding: "2px 6px",
                  borderRadius: "4px",
                  background: "rgba(255, 255, 255, 0.06)",
                  border: "1px solid rgba(255, 255, 255, 0.1)",
                }}
              >
                Hook:{" "}
                {Math.round(
                  candidate.qualityScore.hook <= 1
                    ? candidate.qualityScore.hook * 100
                    : candidate.qualityScore.hook
                )}
                %
              </span>
            )}
            {candidate.qualityScore.contextIndependence !== null && (
              <span
                className="quality-metric-tag"
                title="Standalone context clarity without full video"
                style={{
                  fontSize: "11px",
                  padding: "2px 6px",
                  borderRadius: "4px",
                  background: "rgba(255, 255, 255, 0.06)",
                  border: "1px solid rgba(255, 255, 255, 0.1)",
                }}
              >
                Context:{" "}
                {Math.round(
                  candidate.qualityScore.contextIndependence <= 1
                    ? candidate.qualityScore.contextIndependence * 100
                    : candidate.qualityScore.contextIndependence
                )}
                %
              </span>
            )}
            {candidate.qualityScore.payoff !== null && (
              <span
                className="quality-metric-tag"
                title="Retention sweet-spot & narrative resolution"
                style={{
                  fontSize: "11px",
                  padding: "2px 6px",
                  borderRadius: "4px",
                  background: "rgba(255, 255, 255, 0.06)",
                  border: "1px solid rgba(255, 255, 255, 0.1)",
                }}
              >
                Retention:{" "}
                {Math.round(
                  candidate.qualityScore.payoff <= 1
                    ? candidate.qualityScore.payoff * 100
                    : candidate.qualityScore.payoff
                )}
                %
              </span>
            )}
            {candidate.qualityScore.redundancyPenalty > 0 && (
              <span
                className="quality-metric-tag penalty"
                title="Duplicate window overlap penalty"
                style={{
                  fontSize: "11px",
                  padding: "2px 6px",
                  borderRadius: "4px",
                  background: "rgba(239, 68, 68, 0.15)",
                  border: "1px solid rgba(239, 68, 68, 0.3)",
                  color: "#fca5a5",
                }}
              >
                Overlap Penalty: -
                {Math.round(
                  candidate.qualityScore.redundancyPenalty <= 1
                    ? candidate.qualityScore.redundancyPenalty * 100
                    : candidate.qualityScore.redundancyPenalty
                )}
                %
              </span>
            )}
          </div>
        )}

        {clip?.outputPath && (
          <div className="rendered-clip-path">
            <span className="path-label">Export:</span>
            <span className="path-value truncate">{clip.outputPath}</span>
          </div>
        )}
      </div>

      <div className="moment-card-actions">
        <div className="card-actions-left">
          <button
            className="action-pill-btn social-kit"
            onClick={() => onOpenSocialKit(candidate)}
            title="Generate viral titles, hashtags & captions for this clip"
          >
            <Sparkles size={13} />
            <span>AI Social Kit</span>
          </button>

          <button
            className="action-pill-btn cut-action"
            onClick={() => onCutCandidate(candidate.id)}
            disabled={isBusy || !hasFfmpeg}
            title={
              isCut
                ? "Re-cut this 9:16 vertical clip"
                : "Cut 9:16 vertical clip with stylized captions"
            }
          >
            {isCuttingThis ? (
              <Loader2 className="spin" size={13} />
            ) : (
              <Scissors size={13} />
            )}
            <span>
              {isCuttingThis ? "Cutting..." : isCut ? "Re-cut" : "Cut Clip"}
            </span>
          </button>

          {isCut && clip?.outputPath && (
            <button
              className="action-pill-btn finder"
              onClick={() => onOpenFolder(clip.outputPath!)}
              title="Reveal clip in macOS Finder"
            >
              <FolderOpen size={13} />
              <span>Finder</span>
            </button>
          )}
        </div>

        <div className="card-actions-right">
          <button
            className={`action-pill-btn instagram-publish-btn ${isPublishingThis ? "loading" : ""}`}
            onClick={() => onPublishToInstagram(candidate.id)}
            disabled={isPublishingThis}
            title={
              isPublishingThis
                ? "Publishing clip to Instagram Reels..."
                : igPost?.status === "published"
                  ? "Re-post this clip to Instagram Reels"
                  : "Automatically cut clip, generate AI caption/hashtags, and post to Instagram Reels"
            }
          >
            {isPublishingThis ? (
              <Loader2 className="spin" size={13} />
            ) : (
              <Instagram size={13} />
            )}
            <span>
              {isPublishingThis
                ? isCut
                  ? "Posting..."
                  : "Cutting & Posting..."
                : igPost?.status === "published"
                  ? "Re-post IG"
                  : "Post to Reels"}
            </span>
          </button>

          <button
            className={`action-pill-btn youtube-publish-btn ${isPublishingToYoutube ? "loading" : ""}`}
            onClick={() => onPublishToYouTube(candidate.id)}
            disabled={isPublishingToYoutube}
            title={
              isPublishingToYoutube
                ? "Uploading clip to YouTube Shorts..."
                : ytPost?.status === "published"
                  ? "Re-upload this clip to YouTube Shorts"
                  : "Automatically cut 9:16 clip, generate AI metadata & #Shorts tag, and upload directly to YouTube"
            }
          >
            {isPublishingToYoutube ? (
              <Loader2 className="spin" size={13} />
            ) : (
              <Youtube size={13} />
            )}
            <span>
              {isPublishingToYoutube
                ? isCut
                  ? "Uploading..."
                  : "Cutting & Uploading..."
                : ytPost?.status === "published"
                  ? "Re-post Shorts"
                  : "Post to Shorts"}
            </span>
          </button>
        </div>
      </div>
    </article>
  );
}
