import {
  CheckSquare,
  Square,
  Users,
  Instagram,
  Loader2,
  AlertTriangle,
  Sparkles,
  Scissors,
  FolderOpen,
} from "lucide-react";
import {
  Candidate,
  Clip,
  InstagramPost,
  AppSection,
  ReframeMode,
} from "../../types";
import { PodcastTimelinePreview } from "../podcast/PodcastTimelinePreview";

interface MomentCardProps {
  candidate: Candidate;
  clip?: Clip;
  igPost?: InstagramPost;
  isCuttingThis: boolean;
  isPublishingThis: boolean;
  appSection: AppSection;
  reframeMode: ReframeMode;
  sourcePath: string;
  formatTime: (sec: number) => string;
  onToggleSelect: (id: string) => void;
  onOpenSocialKit: (candidate: Candidate) => void;
  onCutCandidate: (candidateId: string) => void;
  onOpenFolder: (path: string) => void;
  onPublishToInstagram: (candidateId: string) => void;
  onLayoutOverride?: (
    candidateId: string,
    layout: "auto" | "single" | "split_two" | "split_three"
  ) => void;
  hasFfmpeg: boolean;
  isBusy: boolean;
}

export function MomentCard({
  candidate,
  clip,
  igPost,
  isCuttingThis,
  isPublishingThis,
  appSection,
  reframeMode,
  sourcePath,
  formatTime,
  onToggleSelect,
  onOpenSocialKit,
  onCutCandidate,
  onOpenFolder,
  onPublishToInstagram,
  onLayoutOverride,
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
          {reframeMode === "podcast_split" && (
            <span
              className="candidate-podcast-pill"
              title="9:16 Two-Person Table Split Screen"
            >
              <Users size={11} /> 2-Person Split
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

        {clip?.outputPath && (
          <div className="rendered-clip-path">
            <span className="path-label">Export:</span>
            <span className="path-value truncate">{clip.outputPath}</span>
          </div>
        )}

        {(appSection === "podcast" || reframeMode === "podcast_split") && (
          <PodcastTimelinePreview
            sourcePath={sourcePath}
            startSec={candidate.startSec}
            durationSec={candidate.endSec - candidate.startSec}
            initialLayoutOverride={candidate.layoutOverride}
            onLayoutOverride={(layout) =>
              onLayoutOverride?.(candidate.id, layout)
            }
            autoLoad={candidate.selected}
          />
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
        </div>
      </div>
    </article>
  );
}
