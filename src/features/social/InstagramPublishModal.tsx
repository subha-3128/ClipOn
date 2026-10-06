import { Instagram, X, ExternalLink, Loader2, Check, BadgeCheck, AlertTriangle } from "lucide-react";

interface InstagramPublishModalProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string;
  setAccountId: (id: string) => void;
  accessToken: string;
  setAccessToken: (token: string) => void;
  onTestConnection: () => Promise<void>;
  testing: boolean;
  status: { success: boolean; message: string } | null;
  onSaveAndPost: () => Promise<void>;
  saving: boolean;
  pendingCandidateId: string | null;
  onOpenExternal: (url: string) => void;
}

export function InstagramPublishModal({
  isOpen,
  onClose,
  accountId,
  setAccountId,
  accessToken,
  setAccessToken,
  onTestConnection,
  testing,
  status,
  onSaveAndPost,
  saving,
  pendingCandidateId,
  onOpenExternal,
}: InstagramPublishModalProps) {
  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="settings-modal"
        style={{ width: "min(560px, 94vw)" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <div className="modal-header-left">
            <div
              className="modal-icon-badge"
              style={{
                background:
                  "linear-gradient(135deg, rgba(225, 48, 108, 0.2) 0%, rgba(131, 58, 180, 0.2) 100%)",
                color: "#fb7185",
                borderColor: "rgba(225, 48, 108, 0.4)",
              }}
            >
              <Instagram size={18} />
            </div>
            <div>
              <h3>Official Meta Graph API Setup</h3>
              <p>Post high-viral Reels directly to Instagram with AI captions & hashtags</p>
            </div>
          </div>
          <button className="modal-close-btn" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body" style={{ padding: "20px" }}>
          <div
            style={{
              background: "rgba(225, 48, 108, 0.08)",
              border: "1px solid rgba(225, 48, 108, 0.25)",
              borderRadius: "8px",
              padding: "12px 14px",
              marginBottom: "16px",
              fontSize: "12px",
              color: "#fbcfe8",
              lineHeight: "1.5",
            }}
          >
            <strong style={{ color: "#ffffff", display: "block", marginBottom: "4px" }}>
              Meta Graph API Direct Publishing
            </strong>
            Enter your Instagram Professional Account ID and Meta Graph API Access Token. Once entered, ClipOn will automatically render vertical clips, generate engaging AI titles, captions, and hashtags, and publish directly to your Instagram Reels!
          </div>

          <div className="settings-form-stack">
            <div className="settings-field-group">
              <label style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span>Instagram Professional / Creator Account ID</span>
                <button
                  type="button"
                  className="action-pill-btn"
                  style={{ height: "22px", fontSize: "10.5px", padding: "0 6px" }}
                  onClick={() => onOpenExternal("https://developers.facebook.com/tools/explorer/")}
                  title="Open Meta Graph API Explorer in browser"
                >
                  <ExternalLink size={10} />
                  <span>Graph Explorer</span>
                </button>
              </label>
              <input
                type="text"
                value={accountId}
                onChange={(e) => setAccountId(e.target.value)}
                placeholder="e.g. 17841400000000000"
                autoFocus
              />
              <span className="folder-hint">
                Found in Meta Business Suite or via Graph API Explorer (/me/accounts)
              </span>
            </div>

            <div className="settings-field-group">
              <label>Meta User / Page Access Token</label>
              <input
                type="password"
                value={accessToken}
                onChange={(e) => setAccessToken(e.target.value)}
                placeholder="EAA... (Token with instagram_basic and instagram_content_publish permissions)"
              />
              <span className="folder-hint">
                Requires 'instagram_basic' and 'instagram_content_publish' permissions
              </span>
            </div>

            <div style={{ display: "flex", gap: "8px", marginTop: "4px" }}>
              <button
                type="button"
                className="studio-btn secondary small"
                onClick={() => void onTestConnection()}
                disabled={testing}
              >
                {testing ? <Loader2 className="spin" size={12} /> : <Check size={12} />}
                <span>{testing ? "Testing Connection..." : "Test Connection"}</span>
              </button>
            </div>

            {status && (
              <div className={`connection-status-banner ${status.success ? "success" : "error"}`}>
                {status.success ? <BadgeCheck size={16} /> : <AlertTriangle size={16} />}
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
            className="studio-btn instagram-active"
            onClick={() => void onSaveAndPost()}
            disabled={saving}
          >
            {saving ? <Loader2 className="spin" size={14} /> : <Instagram size={14} />}
            <span>
              {saving
                ? "Saving & Posting..."
                : pendingCandidateId
                ? "Save & Post to Instagram"
                : "Save Credentials"}
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
