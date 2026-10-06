import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { XCircle, Loader2, CheckCircle2, AlertCircle } from "lucide-react";
import type { JobInfo } from "../../types";

type JobProgressBarProps = {
  currentJobId?: string | null;
  onJobComplete?: (jobId: string) => void;
  onJobCancel?: (jobId: string) => void;
};

export function JobProgressBar({
  currentJobId,
  onJobComplete,
  onJobCancel,
}: JobProgressBarProps) {
  const [job, setJob] = useState<JobInfo | null>(null);
  const [isCancelling, setIsCancelling] = useState(false);

  useEffect(() => {
    let unlistenFn: (() => void) | undefined;

    const setupListener = async () => {
      unlistenFn = await listen<JobInfo>("job-progress", (event) => {
        const payload = event.payload;
        if (!currentJobId || payload.id === currentJobId) {
          setJob(payload);
          if (payload.state === "Completed") {
            onJobComplete?.(payload.id);
          } else if (payload.state === "Cancelled") {
            setIsCancelling(false);
            onJobCancel?.(payload.id);
          }
        }
      });
    };

    setupListener();

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, [currentJobId, onJobComplete, onJobCancel]);

  if (!job || job.state === "Completed") {
    return null;
  }

  const handleCancel = async () => {
    if (!job.id) return;
    setIsCancelling(true);
    try {
      await invoke("cancel_job", { jobId: job.id });
    } catch (err) {
      console.error("Failed to cancel job:", err);
      setIsCancelling(false);
    }
  };

  const isFailed = job.state === "Failed";
  const isCancelled = job.state === "Cancelled";

  return (
    <div className="job-progress-card" style={{
      background: "linear-gradient(135deg, rgba(30, 41, 59, 0.95), rgba(15, 23, 42, 0.95))",
      border: "1px solid rgba(56, 189, 248, 0.3)",
      borderRadius: "12px",
      padding: "14px 18px",
      margin: "12px 0",
      boxShadow: "0 8px 24px rgba(0, 0, 0, 0.35)",
      backdropFilter: "blur(12px)",
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "8px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          {isFailed ? (
            <AlertCircle size={18} color="#ef4444" />
          ) : isCancelled ? (
            <XCircle size={18} color="#f59e0b" />
          ) : (
            <Loader2 size={18} color="#38bdf8" className="spin" />
          )}
          <div>
            <div style={{ fontSize: "13px", fontWeight: "600", color: "#f8fafc" }}>
              {job.stage}
            </div>
            {job.error && (
              <div style={{ fontSize: "11px", color: "#f87171", marginTop: "2px" }}>
                {job.error}
              </div>
            )}
          </div>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          <span style={{ fontSize: "13px", fontWeight: "700", color: "#38bdf8" }}>
            {job.progress}%
          </span>
          {!isFailed && !isCancelled && (
            <button
              type="button"
              onClick={handleCancel}
              disabled={isCancelling}
              style={{
                background: "rgba(239, 68, 68, 0.15)",
                border: "1px solid rgba(239, 68, 68, 0.4)",
                color: "#fca5a5",
                borderRadius: "6px",
                padding: "4px 10px",
                fontSize: "11px",
                fontWeight: "600",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                gap: "5px",
              }}
            >
              <XCircle size={12} />
              {isCancelling ? "Cancelling..." : "Cancel"}
            </button>
          )}
        </div>
      </div>

      <div style={{
        width: "100%",
        height: "6px",
        background: "rgba(255, 255, 255, 0.1)",
        borderRadius: "999px",
        overflow: "hidden",
      }}>
        <div style={{
          width: `${job.progress}%`,
          height: "100%",
          background: isFailed
            ? "#ef4444"
            : isCancelled
            ? "#f59e0b"
            : "linear-gradient(90deg, #38bdf8, #818cf8)",
          transition: "width 0.3s ease",
        }} />
      </div>
    </div>
  );
}
