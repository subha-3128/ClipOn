import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { XCircle, Loader2, AlertCircle } from "lucide-react";
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
  const [jobs, setJobs] = useState<Map<string, JobInfo>>(new Map());
  const [cancellingIds, setCancellingIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    // Populate active jobs on mount
    invoke<JobInfo[]>("get_active_jobs")
      .then((active) => {
        if (active && active.length > 0) {
          setJobs((prev) => {
            const next = new Map(prev);
            for (const j of active) {
              if (!currentJobId || j.id === currentJobId) {
                next.set(j.id, j);
              }
            }
            return next;
          });
        }
      })
      .catch(() => {});

    let unlistenFn: (() => void) | undefined;

    const setupListener = async () => {
      unlistenFn = await listen<JobInfo>("job-progress", (event) => {
        const payload = event.payload;
        if (!currentJobId || payload.id === currentJobId) {
          setJobs((prev) => {
            const next = new Map(prev);
            if (payload.state === "Completed") {
              onJobComplete?.(payload.id);
              next.delete(payload.id);
            } else if (payload.state === "Cancelled") {
              onJobCancel?.(payload.id);
              next.delete(payload.id);
            } else {
              next.set(payload.id, payload);
            }
            return next;
          });
        }
      });
    };

    setupListener();

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, [currentJobId, onJobComplete, onJobCancel]);

  const handleCancel = async (jobId: string) => {
    setCancellingIds((prev) => new Set(prev).add(jobId));
    try {
      await invoke("cancel_job", { jobId });
    } catch (err) {
      console.error("Failed to cancel job:", err);
      setCancellingIds((prev) => {
        const next = new Set(prev);
        next.delete(jobId);
        return next;
      });
    }
  };

  const activeJobList = Array.from(jobs.values()).filter(
    (j) => j.state !== "Completed"
  );

  if (activeJobList.length === 0) {
    return null;
  }

  return (
    <div className="active-jobs-list">
      {activeJobList.map((job) => {
        const isFailed = job.state === "Failed";
        const isCancelled = job.state === "Cancelled";
        const isCancelling = cancellingIds.has(job.id);

        return (
          <div key={job.id} className="job-item-card">
            <div className="job-item-header">
              <div className="job-item-title">
                {isFailed ? (
                  <AlertCircle size={17} color="var(--danger)" />
                ) : isCancelled ? (
                  <XCircle size={17} color="var(--warning)" />
                ) : (
                  <Loader2
                    size={17}
                    color="var(--accent-cyan)"
                    className="spin"
                  />
                )}
                <div>
                  <div className="job-stage-label">
                    {job.stage || "Processing clip..."}
                  </div>
                  {job.error && (
                    <div className="job-error-label">{job.error}</div>
                  )}
                </div>
              </div>

              <div
                style={{ display: "flex", alignItems: "center", gap: "12px" }}
              >
                <span
                  style={{
                    fontSize: "13px",
                    fontWeight: "700",
                    color: "var(--accent-cyan)",
                  }}
                >
                  {job.progress}%
                </span>
                {!isFailed && !isCancelled && (
                  <button
                    type="button"
                    onClick={() => handleCancel(job.id)}
                    disabled={isCancelling}
                    className="job-item-cancel-btn"
                  >
                    <XCircle size={14} />
                    <span>{isCancelling ? "Cancelling..." : "Cancel"}</span>
                  </button>
                )}
              </div>
            </div>

            <div className="job-item-bar-track">
              <div
                className="job-item-bar-fill"
                style={{
                  width: `${job.progress}%`,
                  background: isFailed
                    ? "var(--danger)"
                    : isCancelled
                      ? "var(--warning)"
                      : "var(--accent)",
                }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
