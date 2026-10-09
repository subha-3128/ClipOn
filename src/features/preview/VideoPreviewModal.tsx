import React, { useRef, useEffect } from "react";
import {
  X,
  Play,
  Sparkles,
  Instagram,
  Youtube,
  FolderOpen,
} from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { AccessibleModal } from "../../components/AccessibleModal";
import type { Candidate, Clip, InstagramPost, YouTubePost } from "../../types";

interface VideoPreviewModalProps {
  candidate: Candidate | null;
  clip?: Clip;
  igPost?: InstagramPost;
  ytPost?: YouTubePost;
  onClose: () => void;
  onOpenSocialKit?: (candidate: Candidate) => void;
  onPublishToInstagram?: (candidateId: string) => void;
  onPublishToYouTube?: (candidateId: string) => void;
  onOpenFolder?: (path: string) => void;
  formatTime?: (sec: number) => string;
}

function defaultFormatTime(sec: number): string {
  if (isNaN(sec) || sec < 0) return "0:00";
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function VideoPreviewModal({
  candidate,
  clip,
  igPost: _igPost,
  ytPost: _ytPost,
  onClose,
  onOpenSocialKit,
  onPublishToInstagram,
  onPublishToYouTube,
  onOpenFolder,
  formatTime = defaultFormatTime,
}: VideoPreviewModalProps) {
  const isOpen = Boolean(candidate && clip?.outputPath);
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (isOpen && videoRef.current) {
      videoRef.current.play().catch(() => {
        // Autoplay policy fallback
      });
    }
  }, [isOpen]);

  if (!isOpen || !candidate || !clip?.outputPath) return null;

  const videoUrl = convertFileSrc(clip.outputPath);
  const durationSec = Math.round(candidate.endSec - candidate.startSec);
  const viralScore = Math.round(
    candidate.score > 1 ? candidate.score : candidate.score * 100
  );

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onClose}
      title={`Preview Clip #${candidate.rank} - ${candidate.hook}`}
      titleId="video-preview-modal-title"
      dialogClassName="video-preview-modal"
    >
      <div className="modal-header">
        <div className="modal-header-left">
          <div className="modal-icon-badge preview">
            <Play size={18} />
          </div>
          <div className="modal-title-stack">
            <div className="preview-title-row">
              <h3 id="video-preview-modal-title" className="truncate">
                {candidate.hook}
              </h3>
              <span className="moment-score-badge highlight">
                {viralScore}% Viral Score
              </span>
            </div>
            <p className="preview-subtitle">
              Rank #{candidate.rank} • {formatTime(candidate.startSec)} –{" "}
              {formatTime(candidate.endSec)} ({durationSec}s) • 9:16 Vertical
            </p>
          </div>
        </div>
        <button
          className="modal-close-btn"
          onClick={onClose}
          aria-label="Close video preview dialog"
        >
          <X size={16} />
        </button>
      </div>

      <div className="modal-body preview-body">
        <div className="video-player-container">
          <video
            ref={videoRef}
            src={videoUrl}
            controls
            playsInline
            className="video-player-element"
          />
          <div className="video-player-overlay-tag">
            <span>9:16 HD</span>
          </div>
        </div>

        <div className="preview-meta-sidebar">
          <div className="preview-info-card">
            <h4>Viral Hook Narrative</h4>
            <p className="preview-rationale">"{candidate.rationale}"</p>
          </div>

          {candidate.qualityScore && (
            <div className="preview-quality-card">
              <h4>Retention Breakdown</h4>
              <div className="quality-meters-list">
                <div className="quality-meter-item">
                  <span className="meter-label">Opening Hook</span>
                  <div className="meter-bar-track">
                    <div
                      className="meter-bar-fill"
                      style={{
                        width: `${Math.round(
                          (candidate.qualityScore.hook ?? 0.8) * 100
                        )}%`,
                      }}
                    />
                  </div>
                  <span className="meter-val">
                    {Math.round((candidate.qualityScore.hook ?? 0.8) * 100)}%
                  </span>
                </div>

                <div className="quality-meter-item">
                  <span className="meter-label">Narrative Payoff</span>
                  <div className="meter-bar-track">
                    <div
                      className="meter-bar-fill green"
                      style={{
                        width: `${Math.round(
                          (candidate.qualityScore.payoff ?? 0.85) * 100
                        )}%`,
                      }}
                    />
                  </div>
                  <span className="meter-val">
                    {Math.round((candidate.qualityScore.payoff ?? 0.85) * 100)}%
                  </span>
                </div>

                <div className="quality-meter-item">
                  <span className="meter-label">Context Clarity</span>
                  <div className="meter-bar-track">
                    <div
                      className="meter-bar-fill purple"
                      style={{
                        width: `${Math.round(
                          (candidate.qualityScore.contextIndependence ?? 0.8) *
                            100
                        )}%`,
                      }}
                    />
                  </div>
                  <span className="meter-val">
                    {Math.round(
                      (candidate.qualityScore.contextIndependence ?? 0.8) * 100
                    )}
                    %
                  </span>
                </div>
              </div>
            </div>
          )}

          <div className="preview-actions-group">
            <h4>Publish & Distribute</h4>
            <div className="preview-actions-grid">
              {onOpenSocialKit && (
                <button
                  className="studio-btn secondary small"
                  onClick={() => {
                    onClose();
                    onOpenSocialKit(candidate);
                  }}
                >
                  <Sparkles size={13} />
                  <span>AI Social Kit</span>
                </button>
              )}

              {onOpenFolder && clip.outputPath && (
                <button
                  className="studio-btn secondary small"
                  onClick={() => onOpenFolder(clip.outputPath!)}
                  title="Reveal file in Finder"
                >
                  <FolderOpen size={13} />
                  <span>Finder</span>
                </button>
              )}

              {onPublishToInstagram && (
                <button
                  className="studio-btn instagram-active small"
                  onClick={() => {
                    onClose();
                    onPublishToInstagram(candidate.id);
                  }}
                >
                  <Instagram size={13} />
                  <span>Post to Reels</span>
                </button>
              )}

              {onPublishToYouTube && (
                <button
                  className="studio-btn youtube-active small"
                  onClick={() => {
                    onClose();
                    onPublishToYouTube(candidate.id);
                  }}
                >
                  <Youtube size={13} />
                  <span>Post to Shorts</span>
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </AccessibleModal>
  );
}
