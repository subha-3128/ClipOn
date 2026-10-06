import React, { createContext, useContext, useState, useCallback } from "react";
import { AppError, createAppError, ErrorSeverity, ErrorAction } from "../../types/error";
import { AlertCircle, AlertTriangle, CheckCircle, Info, X } from "lucide-react";

interface ErrorContextValue {
  errors: AppError[];
  showError: (message: string, options?: { code?: string; details?: string; severity?: ErrorSeverity; recoverable?: boolean; action?: ErrorAction }) => string;
  showWarning: (message: string, details?: string) => string;
  showInfo: (message: string) => string;
  dismissError: (id: string) => void;
  clearAllErrors: () => void;
}

const ErrorContext = createContext<ErrorContextValue | null>(null);

export function ErrorProvider({ children }: { children: React.ReactNode }) {
  const [errors, setErrors] = useState<AppError[]>([]);

  const showError = useCallback(
    (
      message: string,
      options?: {
        code?: string;
        details?: string;
        severity?: ErrorSeverity;
        recoverable?: boolean;
        action?: ErrorAction;
      }
    ) => {
      const err = createAppError(message, options);
      setErrors((prev) => [err, ...prev.slice(0, 4)]); // Keep at most 5 recent errors

      // Auto dismiss non-fatal errors after 8 seconds if no action
      if (err.severity !== "fatal" && !err.action) {
        setTimeout(() => {
          setErrors((prev) => prev.filter((e) => e.id !== err.id));
        }, 8000);
      }
      return err.id;
    },
    []
  );

  const showWarning = useCallback((message: string, details?: string) => {
    return showError(message, { severity: "warning", details });
  }, [showError]);

  const showInfo = useCallback((message: string) => {
    return showError(message, { severity: "info" });
  }, [showError]);

  const dismissError = useCallback((id: string) => {
    setErrors((prev) => prev.filter((e) => e.id !== id));
  }, []);

  const clearAllErrors = useCallback(() => {
    setErrors([]);
  }, []);

  return (
    <ErrorContext.Provider
      value={{
        errors,
        showError,
        showWarning,
        showInfo,
        dismissError,
        clearAllErrors,
      }}
    >
      {children}
      {errors.length > 0 && (
        <div className="error-toast-container" style={{
          position: "fixed",
          bottom: 24,
          right: 24,
          zIndex: 9999,
          display: "flex",
          flexDirection: "column",
          gap: 8,
          maxWidth: 420,
          width: "100%",
          pointerEvents: "none"
        }}>
          {errors.map((err) => (
            <div
              key={err.id}
              className={`error-toast ${err.severity}`}
              style={{
                pointerEvents: "auto",
                background: err.severity === "fatal" || err.severity === "error" ? "rgba(30, 10, 10, 0.95)" : "rgba(25, 25, 35, 0.95)",
                border: `1px solid ${err.severity === "fatal" || err.severity === "error" ? "#ef4444" : err.severity === "warning" ? "#f59e0b" : "#3b82f6"}`,
                borderRadius: 8,
                padding: "12px 16px",
                color: "#f8fafc",
                boxShadow: "0 10px 25px -5px rgba(0, 0, 0, 0.5)",
                backdropFilter: "blur(12px)",
                display: "flex",
                flexDirection: "column",
                gap: 6
              }}
            >
              <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 10 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  {err.severity === "fatal" || err.severity === "error" ? (
                    <AlertCircle size={18} color="#ef4444" style={{ flexShrink: 0 }} />
                  ) : err.severity === "warning" ? (
                    <AlertTriangle size={18} color="#f59e0b" style={{ flexShrink: 0 }} />
                  ) : (
                    <Info size={18} color="#3b82f6" style={{ flexShrink: 0 }} />
                  )}
                  <div style={{ fontWeight: 600, fontSize: 13, lineHeight: "1.3" }}>
                    {err.message}
                  </div>
                </div>
                <button
                  onClick={() => dismissError(err.id)}
                  style={{
                    background: "none",
                    border: "none",
                    color: "#94a3b8",
                    cursor: "pointer",
                    padding: 2,
                    display: "flex",
                    alignItems: "center"
                  }}
                  title="Dismiss"
                >
                  <X size={14} />
                </button>
              </div>

              {err.details && (
                <div style={{ fontSize: 11, color: "#94a3b8", paddingLeft: 26, wordBreak: "break-word" }}>
                  {err.details}
                </div>
              )}

              {err.action && (
                <div style={{ paddingLeft: 26, marginTop: 4 }}>
                  <button
                    onClick={() => {
                      err.action?.onClick();
                      dismissError(err.id);
                    }}
                    style={{
                      background: "rgba(255, 255, 255, 0.1)",
                      border: "1px solid rgba(255, 255, 255, 0.2)",
                      borderRadius: 4,
                      color: "#fff",
                      fontSize: 11,
                      padding: "4px 10px",
                      cursor: "pointer",
                      fontWeight: 500
                    }}
                  >
                    {err.action.label}
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </ErrorContext.Provider>
  );
}

export function useAppError() {
  const context = useContext(ErrorContext);
  if (!context) {
    throw new Error("useAppError must be used within an ErrorProvider");
  }
  return context;
}
