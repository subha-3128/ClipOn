import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Users, User, Clock, RefreshCw, Sliders } from "lucide-react";
import type {
  DynamicPodcastReframingResult,
  LayoutSegment,
  PersonTrack,
} from "../../types";

type PodcastTimelinePreviewProps = {
  sourcePath: string;
  startSec: number;
  durationSec: number;
  initialLayoutOverride?: string | null;
  onLayoutOverride?: (
    layout: "auto" | "single" | "split_two" | "split_three"
  ) => void;
  autoLoad?: boolean;
};

export const PODCAST_TRACKING_RATE_FPS = 3.5;

// Module-level shared cache and in-flight request tracker to prevent duplicate analyses
const previewCache = new Map<string, DynamicPodcastReframingResult>();
const inFlightRequests = new Map<
  string,
  Promise<DynamicPodcastReframingResult>
>();

function getCacheKey(
  sourcePath: string,
  startSec: number,
  durationSec: number
): string {
  return `${sourcePath}::${startSec.toFixed(2)}::${durationSec.toFixed(2)}`;
}

export function PodcastTimelinePreview({
  sourcePath,
  startSec,
  durationSec,
  initialLayoutOverride,
  onLayoutOverride,
  autoLoad = false,
}: PodcastTimelinePreviewProps) {
  const cacheKey = getCacheKey(sourcePath, startSec, durationSec);
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<DynamicPodcastReframingResult | null>(
    () => previewCache.get(cacheKey) || null
  );
  const [isExpanded, setIsExpanded] = useState<boolean>(
    () => autoLoad || previewCache.has(cacheKey)
  );
  const [selectedLayout, setSelectedLayout] = useState<
    "auto" | "single" | "split_two" | "split_three"
  >((initialLayoutOverride as any) || "auto");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setSelectedLayout((initialLayoutOverride as any) || "auto");
  }, [initialLayoutOverride]);

  const loadAnalysis = async (forceRefresh = false) => {
    if (!sourcePath || durationSec <= 0) return;
    const key = getCacheKey(sourcePath, startSec, durationSec);

    if (!forceRefresh && previewCache.has(key)) {
      setData(previewCache.get(key)!);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      if (forceRefresh) {
        previewCache.delete(key);
      }

      let promise = inFlightRequests.get(key);
      if (!promise || forceRefresh) {
        promise = invoke<DynamicPodcastReframingResult>("get_podcast_preview", {
          sourcePath,
          startSec,
          durationSec,
        })
          .then((res) => {
            previewCache.set(key, res);
            inFlightRequests.delete(key);
            return res;
          })
          .catch((err) => {
            inFlightRequests.delete(key);
            throw err;
          });
        inFlightRequests.set(key, promise);
      }

      const res = await promise;
      setData(res);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const key = getCacheKey(sourcePath, startSec, durationSec);
    const cached = previewCache.get(key);
    if (cached) {
      setData(cached);
      setIsExpanded(true);
    } else if (autoLoad) {
      setIsExpanded(true);
      loadAnalysis();
    }
  }, [sourcePath, startSec, durationSec, autoLoad]);

  const people: PersonTrack[] = data?.podcast?.people ?? [];
  const segments: LayoutSegment[] = data?.podcast?.segments ?? [];

  const handleOverrideSelect = (
    layout: "auto" | "single" | "split_two" | "split_three"
  ) => {
    setSelectedLayout(layout);
    onLayoutOverride?.(layout);
  };

  if (!isExpanded && !data) {
    return (
      <div
        className="podcast-preview-card collapsed"
        style={{
          background: "rgba(15, 23, 42, 0.5)",
          border: "1px dashed rgba(56, 189, 248, 0.25)",
          borderRadius: "10px",
          padding: "10px 14px",
          marginTop: "12px",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <Users size={14} color="#38bdf8" />
          <span
            style={{ fontSize: "12px", fontWeight: "600", color: "#cbd5e1" }}
          >
            Podcast Intelligence & Framing
          </span>
          {selectedLayout !== "auto" && (
            <span
              style={{
                fontSize: "11px",
                color: "#38bdf8",
                background: "rgba(56, 189, 248, 0.12)",
                padding: "1px 6px",
                borderRadius: "4px",
                border: "1px solid rgba(56, 189, 248, 0.3)",
              }}
            >
              Layout: {selectedLayout}
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={() => {
            setIsExpanded(true);
            loadAnalysis();
          }}
          disabled={loading}
          style={{
            background: "rgba(56, 189, 248, 0.12)",
            border: "1px solid rgba(56, 189, 248, 0.35)",
            color: "#38bdf8",
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
          <Users size={12} />
          <span>Analyze Layout</span>
        </button>
      </div>
    );
  }

  return (
    <div className="podcast-preview-container">
      <div className="podcast-preview-header">
        <div className="podcast-preview-title">
          <Users size={16} color="var(--accent-cyan)" />
          <span>
            Podcast Intelligence Preview (Dense {PODCAST_TRACKING_RATE_FPS} fps
            Tracking)
          </span>
        </div>
        <button
          type="button"
          onClick={() => loadAnalysis(true)}
          disabled={loading}
          className="podcast-preview-refresh-btn"
        >
          <RefreshCw size={11} className={loading ? "spin" : ""} />
          {loading ? "Analyzing..." : "Refresh Track"}
        </button>
      </div>

      {error && (
        <div
          style={{ fontSize: "12px", color: "#f87171", marginBottom: "10px" }}
        >
          Analysis notice: {error}
        </div>
      )}

      {/* Detected Participants */}
      <div style={{ marginBottom: "12px" }}>
        <div className="podcast-identities-label">
          Persistent Identities (
          {people.length > 0
            ? people.length
            : data?.podcast?.two_faces_detected
              ? 2
              : 1}{" "}
          Present)
        </div>
        <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
          {people.length > 0 ? (
            people.map((p) => (
              <div key={p.id} className="podcast-person-badge">
                <User size={13} color="var(--accent-cyan)" />
                <span style={{ fontWeight: "600" }}>
                  {p.name || `Person ${p.id}`}
                </span>
                <span style={{ fontSize: "10px", color: "var(--muted)" }}>
                  ({p.keyframes.length} pts)
                </span>
              </div>
            ))
          ) : (
            <div style={{ fontSize: "12px", color: "#64748b" }}>
              {loading
                ? "Running dense face clustering & tracking..."
                : "Awaiting analysis keyframes"}
            </div>
          )}
        </div>
      </div>

      {/* Dynamic Layout Segments Timeline */}
      {segments.length > 0 && (
        <div style={{ marginBottom: "14px" }}>
          <div
            style={{
              fontSize: "11px",
              color: "#94a3b8",
              textTransform: "uppercase",
              fontWeight: "600",
              marginBottom: "6px",
            }}
          >
            Layout Transitions ({segments.length} Segments)
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            {segments.map((seg, idx) => (
              <div
                key={idx}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  background: "rgba(30, 41, 59, 0.5)",
                  borderRadius: "6px",
                  padding: "6px 10px",
                  fontSize: "12px",
                  borderLeft:
                    seg.layout_type === "split_three"
                      ? "3px solid #f59e0b"
                      : seg.layout_type === "split_two"
                        ? "3px solid #38bdf8"
                        : "3px solid #10b981",
                }}
              >
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  <Clock size={12} color="#94a3b8" />
                  <span style={{ fontFamily: "monospace", color: "#cbd5e1" }}>
                    {seg.start.toFixed(1)}s – {seg.end.toFixed(1)}s
                  </span>
                </div>
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  <span
                    style={{
                      fontSize: "11px",
                      fontWeight: "600",
                      color:
                        seg.layout_type === "split_three"
                          ? "#fbbf24"
                          : seg.layout_type === "split_two"
                            ? "#38bdf8"
                            : "#34d399",
                    }}
                  >
                    {seg.layout_type === "split_three"
                      ? "3-Person (P1:TL, P2:TR, P3:B)"
                      : seg.layout_type === "split_two"
                        ? "2-Person (P1:Top, P2:Bottom)"
                        : "1-Person (Focused)"}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Manual Layout Correction Override */}
      <div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "6px",
            marginBottom: "6px",
          }}
        >
          <Sliders size={12} color="#94a3b8" />
          <span
            style={{
              fontSize: "11px",
              color: "#94a3b8",
              textTransform: "uppercase",
              fontWeight: "600",
            }}
          >
            Manual Correction & Layout Lock
          </span>
        </div>
        <div style={{ display: "flex", gap: "6px" }}>
          {(["auto", "single", "split_two", "split_three"] as const).map(
            (mode) => (
              <button
                key={mode}
                type="button"
                onClick={() => handleOverrideSelect(mode)}
                style={{
                  flex: 1,
                  padding: "6px 8px",
                  borderRadius: "6px",
                  fontSize: "11px",
                  fontWeight: selectedLayout === mode ? "700" : "500",
                  background:
                    selectedLayout === mode
                      ? "rgba(56, 189, 248, 0.2)"
                      : "rgba(255, 255, 255, 0.04)",
                  border:
                    selectedLayout === mode
                      ? "1px solid #38bdf8"
                      : "1px solid rgba(255, 255, 255, 0.08)",
                  color: selectedLayout === mode ? "#38bdf8" : "#94a3b8",
                  cursor: "pointer",
                }}
              >
                {mode === "auto"
                  ? "Dynamic (AI)"
                  : mode === "single"
                    ? "1-Person"
                    : mode === "split_two"
                      ? "2-Split"
                      : "3-Split"}
              </button>
            )
          )}
        </div>
      </div>
    </div>
  );
}
