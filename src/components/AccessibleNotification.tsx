import React from "react";
import { AlertCircle, AlertTriangle, Info, X } from "lucide-react";
import type { AppError } from "../types";

export interface AccessibleNotificationProps {
  errors: AppError[];
  onDismiss: (id: string) => void;
}

/**
 * Reusable accessible notification primitive:
 * - Uses WAI-ARIA live region for screen readers (aria-live="polite", role="region")
 * - Appropriate ARIA roles ("alert" for errors, "status" for warnings/info)
 * - Keyboard dismissable with clear aria-label
 */
export function AccessibleNotification({
  errors,
  onDismiss,
}: AccessibleNotificationProps) {
  if (errors.length === 0) return null;

  return (
    <div
      className="error-toast-container"
      role="region"
      aria-label="Notifications and Alerts"
      aria-live="polite"
      aria-atomic="false"
      style={{
        position: "fixed",
        bottom: 24,
        right: 24,
        zIndex: 9999,
        display: "flex",
        flexDirection: "column",
        gap: 8,
        maxWidth: 420,
        width: "100%",
        pointerEvents: "none",
      }}
    >
      {errors.map((err) => {
        const isAlert = err.severity === "fatal" || err.severity === "error";
        return (
          <div
            key={err.id}
            role={isAlert ? "alert" : "status"}
            aria-atomic="true"
            className={`error-toast ${err.severity}`}
            style={{
              pointerEvents: "auto",
              background: isAlert
                ? "rgba(30, 10, 10, 0.95)"
                : "rgba(25, 25, 35, 0.95)",
              border: `1px solid ${
                isAlert
                  ? "var(--danger)"
                  : err.severity === "warning"
                    ? "var(--warning)"
                    : "var(--accent-cyan)"
              }`,
              borderRadius: 8,
              padding: "12px 16px",
              color: "var(--text)",
              boxShadow: "0 10px 25px -5px rgba(0, 0, 0, 0.5)",
              backdropFilter: "blur(12px)",
              display: "flex",
              flexDirection: "column",
              gap: 6,
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "flex-start",
                justifyContent: "space-between",
                gap: 10,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                {isAlert ? (
                  <AlertCircle
                    size={18}
                    color="var(--danger)"
                    style={{ flexShrink: 0 }}
                  />
                ) : err.severity === "warning" ? (
                  <AlertTriangle
                    size={18}
                    color="var(--warning)"
                    style={{ flexShrink: 0 }}
                  />
                ) : (
                  <Info
                    size={18}
                    color="var(--accent-cyan)"
                    style={{ flexShrink: 0 }}
                  />
                )}
                <div>
                  <div style={{ fontWeight: 600, fontSize: "13px" }}>
                    {err.message}
                  </div>
                  {err.details && (
                    <div
                      style={{
                        fontSize: "11px",
                        color: "var(--muted)",
                        marginTop: 2,
                      }}
                    >
                      {err.details}
                    </div>
                  )}
                </div>
              </div>

              <button
                type="button"
                onClick={() => onDismiss(err.id)}
                aria-label="Dismiss notification"
                style={{
                  background: "transparent",
                  border: "none",
                  color: "var(--muted)",
                  cursor: "pointer",
                  padding: 2,
                  display: "flex",
                  alignItems: "center",
                }}
              >
                <X size={15} />
              </button>
            </div>

            {err.action && (
              <div
                style={{
                  display: "flex",
                  justifyContent: "flex-end",
                  marginTop: 4,
                }}
              >
                <button
                  type="button"
                  className="studio-btn primary small"
                  onClick={() => {
                    err.action?.onClick();
                    onDismiss(err.id);
                  }}
                >
                  {err.action.label}
                </button>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
