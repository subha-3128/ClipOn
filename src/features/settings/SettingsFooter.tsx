import React from "react";
import { CheckCircle2, AlertCircle, Save, Loader2, ShieldCheck } from "lucide-react";

interface SettingsFooterProps {
  hasUnsavedChanges: boolean;
  isSaving: boolean;
  onCancel: () => void;
  onSave: () => void;
}

export function SettingsFooter({
  hasUnsavedChanges,
  isSaving,
  onCancel,
  onSave,
}: SettingsFooterProps) {
  return (
    <div className="settings-footer-bar">
      <div className="settings-footer-status">
        <div className="settings-footer-security-badge" title="Local file keystore stored in app data dir with 0600 permissions">
          <ShieldCheck size={14} className="security-icon" />
          <span>Local Keystore Encryption</span>
        </div>

        {hasUnsavedChanges ? (
          <div className="unsaved-status-indicator warning">
            <AlertCircle size={13} />
            <span>Unsaved changes pending</span>
          </div>
        ) : (
          <div className="unsaved-status-indicator saved">
            <CheckCircle2 size={13} />
            <span>All settings in sync</span>
          </div>
        )}
      </div>

      <div className="settings-footer-actions">
        <button
          type="button"
          className="studio-btn secondary"
          onClick={onCancel}
          disabled={isSaving}
        >
          Cancel
        </button>

        <button
          type="button"
          className="studio-btn primary"
          onClick={onSave}
          disabled={isSaving}
        >
          {isSaving ? (
            <Loader2 className="spin" size={14} />
          ) : (
            <Save size={14} />
          )}
          <span>Save Changes</span>
        </button>
      </div>
    </div>
  );
}
