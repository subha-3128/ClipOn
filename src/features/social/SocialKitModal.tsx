import {
  Sparkles,
  X,
  Loader2,
  Flame,
  Copy,
  Hash,
  RefreshCw,
} from "lucide-react";
import { Candidate, SocialKit } from "../../types";
import { AccessibleModal } from "../../components/AccessibleModal";

interface SocialKitModalProps {
  candidate: Candidate | null;
  loading: boolean;
  kitData?: SocialKit;
  onClose: () => void;
  onRegenerate: (candidateId: string) => void;
  onShowToast: (message: string) => void;
}

export function SocialKitModal({
  candidate,
  loading,
  kitData,
  onClose,
  onRegenerate,
  onShowToast,
}: SocialKitModalProps) {
  if (!candidate) return null;

  return (
    <AccessibleModal
      isOpen={Boolean(candidate)}
      onClose={onClose}
      title="AI Social Publishing Kit"
      titleId="social-kit-modal-title"
      dialogClassName="social-kit-modal"
    >
      <div className="modal-header">
        <div className="modal-header-left">
          <div className="modal-icon-badge">
            <Sparkles size={18} />
          </div>
          <div>
            <h3 id="social-kit-modal-title">AI Social Publishing Kit</h3>
            <p>
              Viral titles, hashtags &amp; captions for Clip #{candidate.rank}
            </p>
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

      <div className="social-kit-modal-body">
        {loading ? (
          <div className="center-loader-box">
            <Loader2 className="spin" size={28} />
            <p>Generating high-converting viral titles &amp; hashtags...</p>
          </div>
        ) : kitData ? (
          (() => {
            const fullPost = `${kitData.titles[0]}\n\n${kitData.description}\n\n${kitData.callToAction}\n\n${kitData.hashtags.join(" ")}`;
            return (
              <div className="social-kit-content">
                {/* Viral Titles */}
                <div className="social-section">
                  <label className="section-label">
                    <Flame size={13} /> High-CTR Titles (Click to copy)
                  </label>
                  <div className="titles-stack">
                    {kitData.titles.map((title, i) => (
                      <div
                        key={i}
                        role="button"
                        tabIndex={0}
                        className="copy-item-row"
                        onClick={() => {
                          navigator.clipboard.writeText(title);
                          onShowToast("Title copied to clipboard!");
                        }}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            navigator.clipboard.writeText(title);
                            onShowToast("Title copied to clipboard!");
                          }
                        }}
                      >
                        <span>{title}</span>
                        <Copy size={13} className="copy-icon" />
                      </div>
                    ))}
                  </div>
                </div>

                {/* Hashtags */}
                <div className="social-section">
                  <div className="section-header-row">
                    <label className="section-label">
                      <Hash size={13} /> Trending Hashtags
                    </label>
                    <button
                      className="text-action-btn"
                      onClick={() => {
                        navigator.clipboard.writeText(
                          kitData.hashtags.join(" ")
                        );
                        onShowToast("All hashtags copied!");
                      }}
                    >
                      Copy All Tags
                    </button>
                  </div>
                  <div className="hashtag-chips-wrap">
                    {kitData.hashtags.map((tag, i) => (
                      <span
                        key={i}
                        role="button"
                        tabIndex={0}
                        className="hashtag-chip"
                        onClick={() => {
                          navigator.clipboard.writeText(tag);
                          onShowToast(`Copied ${tag}`);
                        }}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            navigator.clipboard.writeText(tag);
                            onShowToast(`Copied ${tag}`);
                          }
                        }}
                        title="Click to copy"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                </div>

                {/* Description & Caption */}
                <div className="social-section">
                  <div className="section-header-row">
                    <label className="section-label">
                      📝 Caption &amp; Description
                    </label>
                    <button
                      className="text-action-btn"
                      onClick={() => {
                        navigator.clipboard.writeText(
                          `${kitData.description}\n\n${kitData.callToAction}`
                        );
                        onShowToast("Caption copied!");
                      }}
                    >
                      Copy Caption
                    </button>
                  </div>
                  <div className="caption-preview-box">
                    <p>{kitData.description}</p>
                    <span className="caption-cta">{kitData.callToAction}</span>
                  </div>
                </div>

                {/* Master Copy Button */}
                <div className="social-master-actions">
                  <button
                    className="studio-btn primary full-width"
                    onClick={() => {
                      navigator.clipboard.writeText(fullPost);
                      onShowToast("Full Social Post Package Copied!");
                    }}
                  >
                    <Copy size={15} />
                    Copy Complete Social Package (TikTok / Reels / Shorts)
                  </button>
                  <button
                    className="studio-btn secondary icon-only"
                    onClick={() => void onRegenerate(candidate.id)}
                    title="Regenerate with AI"
                    aria-label="Regenerate AI kit"
                  >
                    <RefreshCw size={14} />
                  </button>
                </div>
              </div>
            );
          })()
        ) : null}
      </div>
    </AccessibleModal>
  );
}
