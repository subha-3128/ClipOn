import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Users, User, LayoutGrid, Clock, RefreshCw, Sliders } from "lucide-react";
import type { DynamicPodcastReframingResult, LayoutSegment, PersonTrack } from "../../types";

type PodcastTimelinePreviewProps = {
  sourcePath: string;
  startSec: number;
  durationSec: number;
  onLayoutOverride?: (layout: "single" | "split_two" | "split_three") => void;
};

export function PodcastTimelinePreview({
  sourcePath,
  startSec,
  durationSec,
  onLayoutOverride,
}: PodcastTimelinePreviewProps) {
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<DynamicPodcastReframingResult | null>(null);
  const [selectedLayout, setSelectedLayout] = useState<"auto" | "single" | "split_two" | "split_three">("auto");
  const [error, setError] = useState<string | null>(null);

  const loadAnalysis = async () => {
    if (!sourcePath || durationSec <= 0) return;
    setLoading(true);
    setError(null);
    try {
      const res = await invoke<DynamicPodcastReframingResult>("get_podcast_preview", {
        sourcePath,
        startSec,
        durationSec,
      });
      setData(res);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAnalysis();
  }, [sourcePath, startSec, durationSec]);

  const people: PersonTrack[] = data?.podcast?.people ?? [];
  const segments: LayoutSegment[] = data?.podcast?.segments ?? [];

  const handleOverrideSelect = (layout: "auto" | "single" | "split_two" | "split_three") => {
    setSelectedLayout(layout);
    if (layout !== "auto") {
      onLayoutOverride?.(layout);
    }
  };

  return (
    <div className="podcast-preview-card" style={{
      background: "rgba(15, 23, 42, 0.75)",
      border: "1px solid rgba(56, 189, 248, 0.2)",
      borderRadius: "12px",
      padding: "16px",
      marginTop: "12px",
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "12px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <Users size={16} color="#38bdf8" />
          <span style={{ fontSize: "13px", fontWeight: "700", color: "#f8fafc" }}>
            Podcast Intelligence Preview (Dense 3.5 fps Tracking)
          </span>
        </div>
        <button
          type="button"
          onClick={loadAnalysis}
          disabled={loading}
          style={{
            background: "rgba(255, 255, 255, 0.05)",
            border: "1px solid rgba(255, 255, 255, 0.1)",
            color: "#94a3b8",
            borderRadius: "6px",
            padding: "4px 8px",
            fontSize: "11px",
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: "4px",
          }}
        >
          <RefreshCw size={11} className={loading ? "spin" : ""} />
          {loading ? "Analyzing..." : "Refresh Track"}
        </button>
      </div>

      {error && (
        <div style={{ fontSize: "12px", color: "#f87171", marginBottom: "10px" }}>
          Analysis notice: {error}
        </div>
      )}

      {/* Detected Participants */}
      <div style={{ marginBottom: "12px" }}>
        <div style={{ fontSize: "11px", color: "#94a3b8", textTransform: "uppercase", fontWeight: "600", marginBottom: "6px" }}>
          Persistent Identities ({people.length > 0 ? people.length : (data?.podcast?.two_faces_detected ? 2 : 1)} Present)
        </div>
        <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
          {people.length > 0 ? (
            people.map((p) => (
              <div key={p.id} style={{
                background: "rgba(56, 189, 248, 0.1)",
                border: "1px solid rgba(56, 189, 248, 0.3)",
                borderRadius: "6px",
                padding: "4px 10px",
                display: "flex",
                alignItems: "center",
                gap: "6px",
                fontSize: "12px",
                color: "#e2e8f0",
              }}>
                <User size={13} color="#38bdf8" />
                <span style={{ fontWeight: "600" }}>{p.name || `Person ${p.id}`}</span>
                <span style={{ fontSize: "10px", color: "#94a3b8" }}>
                  ({p.keyframes.length} pts)
                </span>
              </div>
            ))
          ) : (
            <div style={{ fontSize: "12px", color: "#64748b" }}>
              {loading ? "Running dense face clustering & tracking..." : "Awaiting analysis keyframes"}
            </div>
          )}
        </div>
      </div>

      {/* Dynamic Layout Segments Timeline */}
      {segments.length > 0 && (
        <div style={{ marginBottom: "14px" }}>
          <div style={{ fontSize: "11px", color: "#94a3b8", textTransform: "uppercase", fontWeight: "600", marginBottom: "6px" }}>
            Layout Transitions ({segments.length} Segments)
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            {segments.map((seg, idx) => (
              <div key={idx} style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                background: "rgba(30, 41, 59, 0.5)",
                borderRadius: "6px",
                padding: "6px 10px",
                fontSize: "12px",
                borderLeft: seg.layout_type === "split_three"
                  ? "3px solid #f59e0b"
                  : seg.layout_type === "split_two"
                  ? "3px solid #38bdf8"
                  : "3px solid #10b981",
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  <Clock size={12} color="#94a3b8" />
                  <span style={{ fontFamily: "monospace", color: "#cbd5e1" }}>
                    {seg.start.toFixed(1)}s – {seg.end.toFixed(1)}s
                  </span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  <span style={{
                    fontSize: "11px",
                    fontWeight: "600",
                    color: seg.layout_type === "split_three" ? "#fbbf24" : seg.layout_type === "split_two" ? "#38bdf8" : "#34d399",
                  }}>
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
        <div style={{ display: "flex", alignItems: "center", gap: "6px", marginBottom: "6px" }}>
          <Sliders size={12} color="#94a3b8" />
          <span style={{ fontSize: "11px", color: "#94a3b8", textTransform: "uppercase", fontWeight: "600" }}>
            Manual Correction & Layout Lock
          </span>
        </div>
        <div style={{ display: "flex", gap: "6px" }}>
          {(["auto", "single", "split_two", "split_three"] as const).map((mode) => (
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
                background: selectedLayout === mode ? "rgba(56, 189, 248, 0.2)" : "rgba(255, 255, 255, 0.04)",
                border: selectedLayout === mode ? "1px solid #38bdf8" : "1px solid rgba(255, 255, 255, 0.08)",
                color: selectedLayout === mode ? "#38bdf8" : "#94a3b8",
                cursor: "pointer",
              }}
            >
              {mode === "auto" ? "Dynamic (AI)" : mode === "single" ? "1-Person" : mode === "split_two" ? "2-Split" : "3-Split"}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
