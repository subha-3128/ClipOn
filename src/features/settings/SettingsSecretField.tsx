import React, { useState } from "react";
import { Eye, EyeOff, ShieldCheck, Trash2 } from "lucide-react";

interface SettingsSecretFieldProps {
  id: string;
  label: string;
  description?: string;
  value: string;
  onChange: (val: string) => void;
  isSavedInKeystore?: boolean;
  onDeleteCredential?: () => void;
  placeholder?: string;
  badge?: React.ReactNode;
}

export function SettingsSecretField({
  id,
  label,
  description,
  value,
  onChange,
  isSavedInKeystore = false,
  onDeleteCredential,
  placeholder,
  badge,
}: SettingsSecretFieldProps) {
  const [showSecret, setShowSecret] = useState(false);

  const hasTypedNewValue = value.trim().length > 0;

  return (
    <div className="settings-field-group">
      <div className="settings-field-header">
        <label htmlFor={id} className="settings-field-label">
          <span>{label}</span>
          {badge}
        </label>

        <div className="settings-field-status">
          {hasTypedNewValue ? (
            <span className="status-pill pending">
              <span className="status-dot amber" />
              Unsaved Edit
            </span>
          ) : isSavedInKeystore ? (
            <span className="status-pill active" title="Credential is saved securely in local keystore (chmod 0600)">
              <ShieldCheck size={12} />
              Saved in Keystore
            </span>
          ) : (
            <span className="status-pill missing">
              Not Configured
            </span>
          )}
        </div>
      </div>

      {description && <p className="settings-field-desc">{description}</p>}

      <div className="settings-secret-input-wrapper">
        <input
          id={id}
          type={showSecret ? "text" : "password"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={
            isSavedInKeystore
              ? "•••••••••••••••• (Leave blank to keep existing key)"
              : placeholder || `Enter ${label}`
          }
          autoComplete="off"
          spellCheck={false}
          className="settings-secret-input"
        />

        <div className="settings-secret-controls">
          <button
            type="button"
            className="settings-secret-icon-btn"
            onClick={() => setShowSecret(!showSecret)}
            title={showSecret ? "Hide secret" : "Reveal secret"}
            aria-label={showSecret ? `Hide ${label}` : `Reveal ${label}`}
          >
            {showSecret ? <EyeOff size={14} /> : <Eye size={14} />}
          </button>

          {isSavedInKeystore && onDeleteCredential && (
            <button
              type="button"
              className="settings-secret-delete-btn"
              onClick={onDeleteCredential}
              title={`Remove ${label} from secure keystore`}
              aria-label={`Delete ${label}`}
            >
              <Trash2 size={13} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
