import React, { createContext, useContext, useState, useCallback } from "react";
import {
  AppError,
  createAppError,
  ErrorSeverity,
  ErrorAction,
} from "../../types/error";
import { AccessibleNotification } from "../../components/AccessibleNotification";

interface ErrorContextValue {
  errors: AppError[];
  showError: (
    message: string,
    options?: {
      code?: string;
      details?: string;
      severity?: ErrorSeverity;
      recoverable?: boolean;
      action?: ErrorAction;
    }
  ) => string;
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

  const showWarning = useCallback(
    (message: string, details?: string) => {
      return showError(message, { severity: "warning", details });
    },
    [showError]
  );

  const showInfo = useCallback(
    (message: string) => {
      return showError(message, { severity: "info" });
    },
    [showError]
  );

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
      <AccessibleNotification errors={errors} onDismiss={dismissError} />
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
