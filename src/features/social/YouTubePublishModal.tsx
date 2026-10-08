import { useState } from "react";
import {
  Youtube,
  X,
  ExternalLink,
  Loader2,
  Check,
  BadgeCheck,
  AlertTriangle,
} from "lucide-react";
import { AccessibleModal } from "../../components/AccessibleModal";

interface YouTubePublishModalProps {
  isOpen: boolean;
  onClose: () => void;
  clientId: string;
  setClientId: (id: string) => void;
  clientSecret: string;
  setClientSecret: (secret: string) => void;
  refreshToken: string;
  setRefreshToken: (token: string) => void;
  onTestConnection: () => Promise<void>;
  testing: boolean;
  status: { success: boolean; message: string } | null;
  onSaveAndPost: (
    titleOverride?: string,
    descriptionOverride?: string,
    privacyStatus?: string
  ) => Promise<void>;
  saving: boolean;
  pendingCandidateId: string | null;
  defaultTitle?: string;
  defaultDescription?: string;
  onOpenExternal: (url: string) => void;
}

export function YouTubePublishModal({
  isOpen,
  onClose,
  clientId,
  setClientId,
  clientSecret,
  setClientSecret,
  refreshToken,
  setRefreshToken,
  onTestConnection,
  testing,
  status,
  onSaveAndPost,
  saving,
  pendingCandidateId,
  defaultTitle,
  defaultDescription,
  onOpenExternal,
}: YouTubePublishModalProps) {
  const [titleOverride, setTitleOverride] = useState(defaultTitle || "");
  const [descriptionOverride, setDescriptionOverride] = useState(
    defaultDescription || ""
  );
  const [privacyStatus, setPrivacyStatus] = useState<string>("public");

  if (!isOpen) return null;

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onClose}
      title="YouTube Data API v3 OAuth2 Setup"
      titleId="yt-publish-modal-title"
      dialogClassName="settings-modal"
    >
      <div className="modal-header">
        <div className="modal-header-left">
          <div
            className="modal-icon-badge"
            style={{
              background:
                "linear-gradient(135deg, rgba(239, 68, 68, 0.2) 0%, rgba(220, 38, 38, 0.3) 100%)",
              color: "#ef4444",
              borderColor: "rgba(239, 68, 68, 0.4)",
            }}
          >
            <Youtube size={18} />
          </div>
          <div>
            <h3 id="yt-publish-modal-title">
              Official YouTube Shorts Direct Upload
            </h3>
            <p>
              Upload vertical 9:16 clips directly to YouTube Shorts with AI
              titles &amp; #Shorts tagging
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

      <div className="modal-body">
        <div className="youtube-banner-box">
          <strong className="youtube-banner-title">
            Official Google YouTube Data API v3
          </strong>
          Enter your Google Cloud OAuth2 Client ID, Client Secret, and Refresh
          Token. ClipOn securely stores these in your local keystore, handles
          resumable video transfers with automatic OAuth token refreshment, and
          attaches required
          <code>#Shorts</code> metadata.
        </div>

        <div className="settings-form-stack">
          <div className="settings-field-group">
            <label
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
              }}
            >
              <span>OAuth2 Client ID</span>
              <button
                type="button"
                className="action-pill-btn"
                style={{
                  height: "22px",
                  fontSize: "10.5px",
                  padding: "0 6px",
                }}
                onClick={() =>
                  onOpenExternal(
                    "https://console.cloud.google.com/apis/credentials"
                  )
                }
                title="Open Google Cloud Console Credentials in browser"
              >
                <ExternalLink size={10} />
                <span>Google Cloud Console</span>
              </button>
            </label>
            <input
              type="text"
              value={clientId}
              onChange={(e) => setClientId(e.target.value)}
              placeholder="e.g. 1234567890-xxx.apps.googleusercontent.com"
              autoFocus
            />
            <span className="folder-hint">
              Created under Google Cloud Console &gt; APIs &amp; Services &gt;
              Credentials
            </span>
          </div>

          <div className="settings-field-group">
            <label>OAuth2 Client Secret</label>
            <input
              type="password"
              value={clientSecret}
              onChange={(e) => setClientSecret(e.target.value)}
              placeholder="GOCSPX-..."
            />
            <span className="folder-hint">
              Associated with your OAuth 2.0 Desktop / Web Client
            </span>
          </div>

          <div className="settings-field-group">
            <label>OAuth2 Refresh Token</label>
            <input
              type="password"
              value={refreshToken}
              onChange={(e) => setRefreshToken(e.target.value)}
              placeholder="1//04..."
            />
            <span className="folder-hint">
              Offline refresh token generated with scope:
              https://www.googleapis.com/auth/youtube.upload
            </span>
          </div>

          {pendingCandidateId && (
            <>
              <div className="settings-field-group">
                <label>Video Title Override (Optional)</label>
                <input
                  type="text"
                  value={titleOverride}
                  onChange={(e) => setTitleOverride(e.target.value)}
                  placeholder="Defaults to candidate hook (#Shorts added automatically)"
                />
              </div>

              <div className="settings-field-group">
                <label>Video Description Override (Optional)</label>
                <textarea
                  value={descriptionOverride}
                  onChange={(e) => setDescriptionOverride(e.target.value)}
                  placeholder="Defaults to AI rationale and viral tags"
                  rows={2}
                  style={{
                    background: "var(--color-bg-secondary, #18181b)",
                    color: "inherit",
                    border: "1px solid var(--color-border, #27272a)",
                    borderRadius: "6px",
                    padding: "8px",
                    fontSize: "12px",
                    resize: "vertical",
                  }}
                />
              </div>

              <div className="settings-field-group">
                <label>Privacy Status</label>
                <select
                  value={privacyStatus}
                  onChange={(e) => setPrivacyStatus(e.target.value)}
                >
                  <option value="public">Public (Instant Live Short)</option>
                  <option value="unlisted">
                    Unlisted (Review Before Public)
                  </option>
                  <option value="private">Private (Only You)</option>
                </select>
              </div>
            </>
          )}

          <div style={{ display: "flex", gap: "8px", marginTop: "4px" }}>
            <button
              type="button"
              className="studio-btn secondary small"
              onClick={() => void onTestConnection()}
              disabled={testing}
            >
              {testing ? (
                <Loader2 className="spin" size={12} />
              ) : (
                <Check size={12} />
              )}
              <span>
                {testing ? "Testing Connection..." : "Test Connection"}
              </span>
            </button>
          </div>

          {status && (
            <div
              className={`connection-status-banner ${status.success ? "success" : "error"}`}
            >
              {status.success ? (
                <BadgeCheck size={16} />
              ) : (
                <AlertTriangle size={16} />
              )}
              <span>{status.message}</span>
            </div>
          )}
        </div>
      </div>

      <div className="modal-footer">
        <button className="studio-btn secondary" onClick={onClose}>
          Cancel
        </button>
        <button
          className="studio-btn youtube-active"
          onClick={() =>
            void onSaveAndPost(
              titleOverride.trim() || undefined,
              descriptionOverride.trim() || undefined,
              privacyStatus
            )
          }
          disabled={saving}
        >
          {saving ? (
            <Loader2 className="spin" size={14} />
          ) : (
            <Youtube size={14} />
          )}
          <span>
            {saving
              ? "Saving & Uploading..."
              : pendingCandidateId
                ? "Save & Upload to YouTube Shorts"
                : "Save Credentials"}
          </span>
        </button>
      </div>
    </AccessibleModal>
  );
}
