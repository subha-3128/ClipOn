export type ErrorSeverity = "info" | "warning" | "error" | "fatal";

export type ErrorAction = {
  label: string;
  onClick: () => void;
};

export type AppError = {
  id: string;
  code: string;
  message: string;
  details?: string;
  severity: ErrorSeverity;
  recoverable: boolean;
  action?: ErrorAction;
  timestamp: number;
};

export function createAppError(
  message: string,
  options: {
    code?: string;
    details?: string;
    severity?: ErrorSeverity;
    recoverable?: boolean;
    action?: ErrorAction;
  } = {}
): AppError {
  return {
    id: `err_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`,
    code: options.code || "APP_ERROR",
    message,
    details: options.details,
    severity: options.severity || "error",
    recoverable: options.recoverable ?? true,
    action: options.action,
    timestamp: Date.now(),
  };
}
