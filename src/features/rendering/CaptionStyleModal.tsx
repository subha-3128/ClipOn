import { Captions, X } from "lucide-react";
import { AccessibleModal } from "../../components/AccessibleModal";

interface CaptionStyleModalProps {
  isOpen: boolean;
  selectedStyle: string;
  onSelectStyle: (style: string) => void;
  onConfirm: (style: string) => void;
  onCancel: () => void;
}

export function CaptionStyleModal({
  isOpen,
  selectedStyle,
  onSelectStyle,
  onConfirm,
  onCancel,
}: CaptionStyleModalProps) {
  if (!isOpen) return null;

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onCancel}
      title="Choose Caption Style"
      titleId="caption-style-modal-title"
      dialogClassName="style-modal"
    >
      <div className="modal-header">
        <div className="modal-header-left">
          <div className="modal-icon-badge">
            <Captions size={18} />
          </div>
          <div>
            <h3 id="caption-style-modal-title">Choose Caption Style</h3>
            <p>Select automated subtitle typography for your vertical clips</p>
          </div>
        </div>
        <button
          className="modal-close-btn"
          onClick={onCancel}
          aria-label="Close dialog"
        >
          <X size={16} />
        </button>
      </div>

      <div
        className="style-grid"
        role="radiogroup"
        aria-labelledby="caption-style-modal-title"
      >
        {/* Pro Feature: Hormozi Kinetic Karaoke */}
        <div
          role="radio"
          aria-checked={selectedStyle === "hormozi-kinetic"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "hormozi-kinetic" ? "selected" : ""}`}
          onClick={() => onSelectStyle("hormozi-kinetic")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("hormozi-kinetic");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-hormozi" style={{ color: "#00E6FF" }}>
              KINETIC POP ⚡
            </span>
          </div>
          <div className="style-card-title">Hormozi Kinetic (Pro Karaoke)</div>
          <div className="style-card-desc">
            Word-by-word active highlighting with vibrant yellow &amp; electric
            green keyword pops!
          </div>
        </div>

        {/* Feature 3: Submagic Viral Style with Auto-Emoji */}
        <div
          role="radio"
          aria-checked={selectedStyle === "submagic-viral"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "submagic-viral" ? "selected" : ""}`}
          onClick={() => onSelectStyle("submagic-viral")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("submagic-viral");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-submagic">VIRAL MONEY 💰</span>
          </div>
          <div className="style-card-title">Submagic Viral (Auto-Emoji)</div>
          <div className="style-card-desc">
            High-retention neon yellow text in a dark pillbox with automated
            emojis (🔥, 💰, 🤯, 🚀)!
          </div>
        </div>

        {/* Feature 3: Hormozi Punch Style */}
        <div
          role="radio"
          aria-checked={selectedStyle === "hormozi-punch"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "hormozi-punch" ? "selected" : ""}`}
          onClick={() => onSelectStyle("hormozi-punch")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("hormozi-punch");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-hormozi">CRAZY GAINS 🔥</span>
          </div>
          <div className="style-card-title">Hormozi Punch (Auto-Emoji)</div>
          <div className="style-card-desc">
            Heavy black text on solid golden yellow box with high-impact
            auto-emojis.
          </div>
        </div>

        {/* Feature 3: Neon Cyber Glow */}
        <div
          role="radio"
          aria-checked={selectedStyle === "neon-glow"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "neon-glow" ? "selected" : ""}`}
          onClick={() => onSelectStyle("neon-glow")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("neon-glow");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-neonglow">TECH SHIFT 🚀</span>
          </div>
          <div className="style-card-title">Neon Glow (Auto-Emoji)</div>
          <div className="style-card-desc">
            Electric glowing cyan text with deep shadows and contextual
            auto-emojis.
          </div>
        </div>

        <div
          role="radio"
          aria-checked={selectedStyle === "modern-box"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "modern-box" ? "selected" : ""}`}
          onClick={() => onSelectStyle("modern-box")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("modern-box");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-box">BRAINFOOD BECAUSE</span>
          </div>
          <div className="style-card-title">Modern Box</div>
          <div className="style-card-desc">
            Sleek white text inside semi-transparent dark box. Clean &amp;
            readable.
          </div>
        </div>

        <div
          role="radio"
          aria-checked={selectedStyle === "classic-outline"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "classic-outline" ? "selected" : ""}`}
          onClick={() => onSelectStyle("classic-outline")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("classic-outline");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-outline">BRAINFOOD BECAUSE</span>
          </div>
          <div className="style-card-title">Classic Outline</div>
          <div className="style-card-desc">
            Vibrant bold yellow text with a clean black stroke (CapCut style).
          </div>
        </div>

        <div
          role="radio"
          aria-checked={selectedStyle === "minimal-shadow"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "minimal-shadow" ? "selected" : ""}`}
          onClick={() => onSelectStyle("minimal-shadow")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("minimal-shadow");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-shadow">BRAINFOOD BECAUSE</span>
          </div>
          <div className="style-card-title">Minimal Shadow</div>
          <div className="style-card-desc">
            Pure white text with a soft drop shadow. Elegant &amp; unobtrusive.
          </div>
        </div>

        <div
          role="radio"
          aria-checked={selectedStyle === "vibrant-cyan"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "vibrant-cyan" ? "selected" : ""}`}
          onClick={() => onSelectStyle("vibrant-cyan")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("vibrant-cyan");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-cyan">BRAINFOOD BECAUSE</span>
          </div>
          <div className="style-card-title">Vibrant Cyan</div>
          <div className="style-card-desc">
            Vibrant electric cyan text for tech and modern content.
          </div>
        </div>

        <div
          role="radio"
          aria-checked={selectedStyle === "vibrant-yellow-box"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "vibrant-yellow-box" ? "selected" : ""}`}
          onClick={() => onSelectStyle("vibrant-yellow-box")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("vibrant-yellow-box");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-yellow-box">BRAINFOOD BECAUSE</span>
          </div>
          <div className="style-card-title">Vibrant Yellow Box</div>
          <div className="style-card-desc">
            Bold dark text inside a solid yellow box. High contrast.
          </div>
        </div>

        <div
          role="radio"
          aria-checked={selectedStyle === "vibrant-green"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "vibrant-green" ? "selected" : ""}`}
          onClick={() => onSelectStyle("vibrant-green")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("vibrant-green");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-green">BRAINFOOD BECAUSE</span>
          </div>
          <div className="style-card-title">Vibrant Green</div>
          <div className="style-card-desc">
            High-energy neon green with black borders (Hormozi style).
          </div>
        </div>

        <div
          role="radio"
          aria-checked={selectedStyle === "vibrant-red"}
          tabIndex={0}
          className={`style-card ${selectedStyle === "vibrant-red" ? "selected" : ""}`}
          onClick={() => onSelectStyle("vibrant-red")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelectStyle("vibrant-red");
            }
          }}
        >
          <div className="style-preview-box">
            <span className="preview-text-red">BRAINFOOD BECAUSE</span>
          </div>
          <div className="style-card-title">Vibrant Red</div>
          <div className="style-card-desc">
            Dramatic neon crimson for punchy, dramatic hooks.
          </div>
        </div>
      </div>

      <div className="modal-footer">
        <button className="studio-btn secondary" onClick={onCancel}>
          Cancel
        </button>
        <button
          className="studio-btn primary"
          onClick={() => onConfirm(selectedStyle)}
        >
          Confirm &amp; Import
        </button>
      </div>
    </AccessibleModal>
  );
}
